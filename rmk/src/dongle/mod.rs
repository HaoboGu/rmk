//! Tri-mode dongle firmware (`dongle` feature): a BLE central that relays
//! bonded RMK keyboards to a USB host.
//!
//! The dongle plays three roles at once — HID-over-GATT client toward each
//! keyboard, Rynk GATT client toward the config target, and Rynk server on USB
//! CDC ([`DongleRouter`]). Keymaps and storage stay on the keyboards; the
//! dongle persists only its slot table (bond + name + recency).
//!
//! Task layout (all joined by [`Dongle::run`]):
//! - `ble_task`: trouble runner with the seeking-advertisement scan handler;
//! - `pairing_manager`: the power-on / authorized pairing windows;
//! - one `link_task` per configured link, claiming slots and serving them;
//! - bond-removal housekeeping for forgotten/evicted slots.

pub(crate) mod link;
pub(crate) mod merge;
pub(crate) mod router;

use core::cell::RefCell;

use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy, LeSetScanParams};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use bt_hci::param::{AddrKind, BdAddr};
use embassy_futures::join::{join_array, join4};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_deadline};
use rmk_types::ble::{DONGLE_SEEKING_ADV_KIND, RMK_ADV_COMPANY_ID};
use rmk_types::protocol::rynk::{
    DONGLE_SLOT_NAME_SIZE, DongleSlot as DongleSlotPayload, DongleSlots as DongleSlotsPayload, ProtocolVersion,
};
pub use router::DongleRouter;
use trouble_host::prelude::*;

use crate::channel::FLASH_CHANNEL;
use crate::core_traits::Runnable;
use crate::storage::{DongleSlotMeta, FlashOperationMessage};
use crate::{DONGLE_LINKS_NUM, DONGLE_PAIRING_WINDOW_SECS, DONGLE_SLOTS_NUM, RawMutex};

/// Connection budget of the dongle role, independent of the keyboard role's
/// `CONNECTIONS_MAX` — in one build each binary sizes its own stack.
const DONGLE_CONNECTIONS_MAX: usize = DONGLE_LINKS_NUM;
const DONGLE_L2CAP_CHANNELS_MAX: usize = DONGLE_CONNECTIONS_MAX * 4; // Signal + att + smp + hid

/// BLE resources sized for the dongle role; owned by [`Dongle::run`].
type DongleBleResources = HostResources<DefaultPacketPool, DONGLE_CONNECTIONS_MAX, DONGLE_L2CAP_CHANNELS_MAX>;

/// Slot table restored from storage: one `(bond, meta)` entry per slot.
pub type DongleSlotsInit = heapless::Vec<Option<(BondInformation, DongleSlotMeta)>, DONGLE_SLOTS_NUM>;

/// Which link task, if any, owns a slot right now.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum LinkState {
    Free,
    /// Link task N is connecting/securing/handshaking this slot.
    Claimed(u8),
    /// Live and serving on link task N.
    Connected(u8),
}

/// One remembered keyboard.
pub(crate) struct Slot {
    pub(crate) bond: Option<BondInformation>,
    /// Captured from `GetDeviceInfo` during the pairing handshake; persisted.
    pub(crate) name: heapless::String<DONGLE_SLOT_NAME_SIZE>,
    /// Logical recency (Lamport-style, not a timestamp), bumped on connect and
    /// clean disconnect; the smallest unconnected slot is the eviction victim.
    pub(crate) last_seen: u32,
    pub(crate) link: LinkState,
}

impl Slot {
    const fn empty() -> Self {
        Self {
            bond: None,
            name: heapless::String::new(),
            last_seen: 0,
            link: LinkState::Free,
        }
    }
}

pub(crate) struct SlotTable {
    pub(crate) slots: [Slot; DONGLE_SLOTS_NUM],
    /// Config target set via `SelectDongleTarget`; `None` = implicit rule
    /// (the only bonded slot, or no target).
    pub(crate) explicit_target: Option<u8>,
    /// Keyboard picked by the pairing window, waiting for a link task to pair it.
    pub(crate) pending_pair: Option<(AddrKind, BdAddr)>,
}

impl SlotTable {
    /// Where config traffic goes: the explicit target if still bonded, else
    /// the only bonded slot, else nothing (multi-slot ambiguity).
    pub(crate) fn config_target(&self) -> Option<u8> {
        if let Some(s) = self.explicit_target
            && self.slots.get(s as usize).is_some_and(|slot| slot.bond.is_some())
        {
            return Some(s);
        }
        let mut bonded = self.slots.iter().enumerate().filter(|(_, s)| s.bond.is_some());
        match (bonded.next(), bonded.next()) {
            (Some((i, _)), None) => Some(i as u8),
            _ => None,
        }
    }

    /// Next recency value: a total order is all eviction needs (design §4.8).
    pub(crate) fn bump_last_seen(&mut self, idx: usize) -> u32 {
        let next = self
            .slots
            .iter()
            .map(|s| s.last_seen)
            .max()
            .unwrap_or(0)
            .wrapping_add(1);
        self.slots[idx].last_seen = next;
        next
    }

    /// Slot for a fresh pairing: a free one, else evict the least-recently-seen
    /// unowned slot. `None` only when every slot is currently owned by a link.
    pub(crate) fn allocate(&mut self) -> Option<usize> {
        if let Some(i) = self
            .slots
            .iter()
            .position(|s| s.bond.is_none() && s.link == LinkState::Free)
        {
            return Some(i);
        }
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.link == LinkState::Free)
            .min_by_key(|(_, s)| s.last_seen)
            .map(|(i, _)| i)
    }
}

static SLOT_TABLE: BlockingMutex<RawMutex, RefCell<SlotTable>> = BlockingMutex::new(RefCell::new(SlotTable {
    slots: [const { Slot::empty() }; DONGLE_SLOTS_NUM],
    explicit_target: None,
    pending_pair: None,
}));

/// Read-only slot table access.
pub(crate) fn read_slots<R>(f: impl FnOnce(&SlotTable) -> R) -> R {
    SLOT_TABLE.lock(|t| f(&t.borrow()))
}

/// Mutating slot table access; wakes the router session's topic push.
pub(crate) fn update_slots<R>(f: impl FnOnce(&mut SlotTable) -> R) -> R {
    let r = SLOT_TABLE.lock(|t| f(&mut t.borrow_mut()));
    SLOTS_CHANGED.signal(());
    r
}

/// Like [`update_slots`], for router-internal bookkeeping that must not
/// re-trigger the router's own change handler.
pub(crate) fn update_slots_quiet<R>(f: impl FnOnce(&mut SlotTable) -> R) -> R {
    SLOT_TABLE.lock(|t| f(&mut t.borrow_mut()))
}

/// Slot membership/connection change, consumed by the CDC session (single consumer).
pub(crate) static SLOTS_CHANGED: Signal<RawMutex, ()> = Signal::new();

/// Signaled by a keyboard's `dongle_ctrl` authorization notify (open a window).
pub(crate) static AUTH_WINDOW_SIGNAL: Signal<RawMutex, ()> = Signal::new();

/// Latest matching seeking advertisement seen by the scan handler.
static SEEKER_FOUND: Signal<RawMutex, ((AddrKind, BdAddr), i8)> = Signal::new();

/// Bonds to drop from the trouble stack (forgotten or evicted slots); consumed
/// by a housekeeping task in [`Dongle::run`], since only it holds the stack.
pub(crate) static REMOVED_BONDS: Channel<RawMutex, Identity, 4> = Channel::new();

// Scan/initiate arbitration, same shape as the split central's: whoever holds
// SCANNING_MUTEX owns the controller's scanner; a connector interrupts the
// pairing scan via STOP_SCANNING.
pub(crate) static STACK_STARTED: Signal<RawMutex, bool> = Signal::new();
pub(crate) static STOP_SCANNING: Signal<RawMutex, ()> = Signal::new();
pub(crate) static SCANNING_MUTEX: Mutex<RawMutex, ()> = Mutex::new(());

/// Wait for the BLE runner to start (split central precedent).
pub(crate) async fn wait_for_stack_started() {
    loop {
        if STACK_STARTED.signaled() {
            Timer::after_millis(500).await;
            break;
        }
        Timer::after_millis(500).await;
    }
}

/// Wire snapshot of the slot table for `GetDongleSlots` / `DongleSlotsChange`.
pub(crate) fn slots_snapshot() -> DongleSlotsPayload {
    read_slots(|t| {
        // Dense: the index is the slot number the host addresses, so unbonded
        // slots keep their place instead of shifting the ones behind them.
        let mut slots = heapless::Vec::new();
        for s in t.slots.iter() {
            let _ = slots.push(s.bond.is_some().then(|| DongleSlotPayload {
                connected: matches!(s.link, LinkState::Connected(_)),
                name: s.name.clone(),
            }));
        }
        DongleSlotsPayload {
            slots,
            target: t.config_target(),
        }
    })
}

/// Persist a slot's name + recency beside its bond.
pub(crate) async fn persist_slot_meta(slot: u8, name: heapless::String<DONGLE_SLOT_NAME_SIZE>, last_seen: u32) {
    FLASH_CHANNEL
        .send(FlashOperationMessage::DongleSlotMeta {
            slot,
            meta: DongleSlotMeta { name, last_seen },
        })
        .await;
}

/// Match a dongle-seeking advertisement: `MSD { 0xe118, [0xD0, our major] }`.
fn seeking_adv_matches(data: &[u8]) -> bool {
    let mut i = 0;
    while i + 1 < data.len() {
        let len = data[i] as usize;
        if len == 0 {
            return false;
        }
        let end = i + 1 + len;
        if end > data.len() {
            return false;
        }
        // AD type 0xFF: company id (LE) + [kind, major] payload.
        if data[i + 1] == 0xFF
            && len >= 5
            && data[i + 2..i + 4] == RMK_ADV_COMPANY_ID.to_le_bytes()
            && data[i + 4] == DONGLE_SEEKING_ADV_KIND
            && data[i + 5] == ProtocolVersion::CURRENT.major
        {
            return true;
        }
        i = end;
    }
    false
}

/// Runner event handler: surface seeking keyboards to the pairing window.
struct DongleScanHandler;

impl EventHandler for DongleScanHandler {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = it.next() {
            if seeking_adv_matches(report.data) {
                debug!("[dongle] seeking keyboard {:?} rssi {}", report.addr, report.rssi);
                SEEKER_FOUND.signal(((report.addr_kind, report.addr), report.rssi));
            }
        }
    }
}

async fn ble_task<C: Controller>(mut runner: Runner<'_, C, DefaultPacketPool>) -> ! {
    loop {
        STACK_STARTED.signal(true);
        if runner.run_with_handler(&DongleScanHandler).await.is_err() {
            error!("[dongle] ble runner error");
            Timer::after_millis(100).await;
        }
    }
}

/// One pairing window: scan for seeking keyboards, pick the strongest within
/// 2s of the first sighting, and hand it to a link task via `pending_pair`.
async fn run_pairing_window<C>(stack: &Stack<'_, C, DefaultPacketPool>)
where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
{
    info!("[dongle] pairing window open for {}s", DONGLE_PAIRING_WINDOW_SECS);
    SEEKER_FOUND.reset();
    let deadline = Instant::now() + Duration::from_secs(DONGLE_PAIRING_WINDOW_SECS as u64);

    let scan = async {
        loop {
            let mut central = stack.central();
            wait_for_stack_started().await;
            let mut scanner = Scanner::new(&mut central);
            let scan_config = ScanConfig {
                active: false,
                interval: Duration::from_millis(100),
                window: Duration::from_millis(60),
                ..Default::default()
            };
            let _guard = SCANNING_MUTEX.lock().await;
            match scanner.scan(&scan_config).await {
                // Hold the scan until a link task asks for the controller.
                Ok(_session) => STOP_SCANNING.wait().await,
                Err(_) => Timer::after_millis(500).await,
            }
        }
    };
    let pick = async {
        let (mut best_addr, mut best_rssi) = SEEKER_FOUND.wait().await;
        // Don't sit out the whole window: gather 2s past the first sighting.
        let gather = Instant::now() + Duration::from_secs(2);
        while let Ok((addr, rssi)) = with_deadline(gather, SEEKER_FOUND.wait()).await {
            if rssi > best_rssi {
                (best_addr, best_rssi) = (addr, rssi);
            }
        }
        (best_addr, best_rssi)
    };

    match with_deadline(deadline, select(scan, pick)).await {
        Ok(Either::Second((addr, rssi))) => {
            info!("[dongle] pairing candidate {:?} (rssi {})", addr.1, rssi);
            update_slots(|t| t.pending_pair = Some(addr));
        }
        _ => info!("[dongle] pairing window closed, no keyboard found"),
    }
}

async fn pairing_manager<C>(stack: &Stack<'_, C, DefaultPacketPool>) -> !
where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
{
    // Power-on window: plugging the dongle in *is* the pairing gesture (§2.3).
    run_pairing_window(stack).await;
    loop {
        AUTH_WINDOW_SIGNAL.wait().await;
        run_pairing_window(stack).await;
    }
}

/// The tri-mode dongle runnable. Owns and sizes its own BLE stack — the
/// keyboard role's [`crate::ble::BleTransport`] is not involved, so one
/// build can carry both kinds of binaries. The USB side is a stock
/// [`crate::usb::UsbTransport`] with [`DongleRouter`] attached.
pub struct Dongle<C> {
    /// Taken by `run`, which owns the stack and its resources.
    controller: Option<C>,
    address: [u8; 6],
}

impl<C> Dongle<C> {
    /// `slots` comes from [`crate::storage::new_storage_for_dongle`].
    pub fn new(controller: C, address: [u8; 6], slots: DongleSlotsInit) -> Self {
        update_slots(|t| {
            for (i, entry) in slots.into_iter().enumerate() {
                if let Some((bond, meta)) = entry {
                    info!(
                        "[dongle] slot {}: bonded to {:?} ({})",
                        i,
                        bond.identity.addr,
                        meta.name.as_str()
                    );
                    t.slots[i] = Slot {
                        bond: Some(bond),
                        name: meta.name,
                        last_seen: meta.last_seen,
                        link: LinkState::Free,
                    };
                }
            }
        });
        Self {
            controller: Some(controller),
            address,
        }
    }

    /// The CDC router, for [`crate::usb::UsbTransport::with_dongle_router`].
    pub fn router(&self) -> &'static DongleRouter {
        &DongleRouter
    }
}

impl<C> Runnable for Dongle<C>
where
    C: Controller
        + ControllerCmdAsync<LeSetPhy>
        + ControllerCmdSync<LeReadLocalSupportedFeatures>
        + ControllerCmdSync<LeSetScanParams>,
{
    async fn run(&mut self) -> ! {
        let controller = self.controller.take().expect("Dongle::run called twice");
        let mut resources = DongleBleResources::new();
        let stack = trouble_host::new(controller, &mut resources)
            .set_random_address(Address::random(self.address))
            .build();
        let stack = &stack;

        // Register the persisted bonds with the freshly built stack.
        let bonds: heapless::Vec<BondInformation, DONGLE_SLOTS_NUM> =
            read_slots(|t| t.slots.iter().filter_map(|s| s.bond.clone()).collect());
        for bond in bonds {
            if let Err(e) = stack.add_bond_information(bond) {
                warn!("[dongle] add bond error: {:?}", e);
            }
        }

        let links = join_array(core::array::from_fn::<_, DONGLE_LINKS_NUM, _>(|i| {
            link::link_task(i as u8, stack)
        }));
        let housekeeping = async {
            loop {
                let identity = REMOVED_BONDS.receive().await;
                if let Err(e) = stack.remove_bond_information(identity) {
                    debug!("[dongle] remove bond error: {:?}", e);
                }
            }
        };
        join4(ble_task(stack.runner()), pairing_manager(stack), links, housekeeping).await;
        unreachable!("Dongle sub-tasks must run forever")
    }
}
