#[cfg(feature = "dfu_lock")]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "dfu_lock")]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
#[cfg(feature = "dfu_lock")]
use embassy_sync::signal::Signal;
use embassy_usb::control::{InResponse, OutResponse, Request};
use embassy_usb::driver::Driver;
use embassy_usb::types::StringIndex;
use embassy_usb::{Builder, Handler};
use static_cell::StaticCell;

#[cfg(feature = "dfu_lock")]
use crate::core_traits::Runnable;

// ---------------------------------------------------------------------------
// Chip modules
// ---------------------------------------------------------------------------

#[cfg(feature = "dfu_nrf")]
mod nrf;
#[cfg(feature = "dfu_rp")]
mod rp;

#[cfg(feature = "dfu_nrf")]
pub use self::nrf::{DFU_WRITE_SIZE, get_manager, init_flash, mark_booted};
#[cfg(feature = "dfu_rp")]
pub use self::rp::{DFU_WRITE_SIZE, get_manager, init_flash, mark_booted};

// ---------------------------------------------------------------------------
// Chip-specific type aliases
// ---------------------------------------------------------------------------

/// DFU transfer block size in bytes. Larger values speed up firmware
/// downloads. Must match the USB control buffer size used by the host.
pub const BLOCK_SIZE_DFU: usize = 512;

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use embassy_embedded_hal::flash::partition::BlockingPartition;

#[cfg(feature = "dfu_nrf")]
use self::nrf::{MutexType, PartitionType};
#[cfg(feature = "dfu_rp")]
use self::rp::{MutexType, PartitionType};

// ---------------------------------------------------------------------------
// DfuFlashManager — shared by RP2040 and nRF
// ---------------------------------------------------------------------------

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub struct DfuFlashManager {
    flash_mutex: &'static MutexType,
    state_offset: u32,
    state_size: u32,
    dfu_offset: u32,
    dfu_size: u32,
    storage_offset: u32,
    storage_size: u32,
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl DfuFlashManager {
    pub(super) fn new(
        flash_mutex: &'static MutexType,
        storage_offset: u32,
        storage_size: u32,
        state_offset: u32,
        state_size: u32,
        dfu_offset: u32,
        dfu_size: u32,
    ) -> Self {
        Self {
            flash_mutex,
            state_offset,
            state_size,
            dfu_offset,
            dfu_size,
            storage_offset,
            storage_size,
        }
    }

    pub fn state_partition(&self) -> PartitionType {
        BlockingPartition::new(self.flash_mutex, self.state_offset, self.state_size)
    }

    pub fn dfu_partition(&self) -> PartitionType {
        BlockingPartition::new(self.flash_mutex, self.dfu_offset, self.dfu_size)
    }

    pub fn storage_partition(&self) -> PartitionType {
        BlockingPartition::new(self.flash_mutex, self.storage_offset, self.storage_size)
    }
}

// ---------------------------------------------------------------------------
// DfuStringProvider
// ---------------------------------------------------------------------------

struct DfuStringProvider {
    string_idx: StringIndex,
    string_val: &'static str,
}

impl Handler for DfuStringProvider {
    fn control_out(&mut self, _req: Request, _data: &[u8]) -> Option<OutResponse> {
        None
    }
    fn control_in<'a>(&'a mut self, _req: Request, _buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        None
    }
    fn get_string(&mut self, index: StringIndex, _lang_id: u16) -> Option<&'static str> {
        (index == self.string_idx).then_some(self.string_val)
    }
}

// ---------------------------------------------------------------------------
// DFU lock state
// ---------------------------------------------------------------------------

#[cfg(feature = "dfu_lock")]
static DFU_LOCKED: AtomicBool = AtomicBool::new(true);
#[cfg(feature = "dfu_lock")]
static DFU_STARTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "dfu_lock")]
static DFU_UNLOCK_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[cfg(feature = "dfu_lock")]
pub fn is_dfu_unlocked() -> bool {
    !DFU_LOCKED.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------
// RmkDfuHandler
// ---------------------------------------------------------------------------

#[cfg(feature = "dfu")]
use embassy_usb::class::dfu::{
    consts::Status,
    dfu_mode::{self, DfuState},
};
#[cfg(any(feature = "dfu", feature = "dfu_lock"))]
use rmk_types::dfu::DfuStatus;

/// DFU handler wrapper that blinks an LED during transfer and checks the
/// DFU lock (if `dfu_lock` feature is enabled).
#[cfg(any(feature = "dfu", feature = "dfu_lock"))]
use crate::event::publish_event;

#[cfg(feature = "dfu")]
struct RmkDfuHandler<H> {
    inner: H,
}

#[cfg(feature = "dfu")]
impl<H: dfu_mode::Handler> dfu_mode::Handler for RmkDfuHandler<H> {
    fn start(&mut self) -> Result<(), Status> {
        #[cfg(feature = "dfu_lock")]
        if !is_dfu_unlocked() {
            DFU_UNLOCK_SIGNAL.signal(());
            info!("dfu_lock: DFU download rejected — keys not unlocked");
            return Err(Status::ErrVendor);
        }
        #[cfg(feature = "dfu_lock")]
        DFU_STARTED.store(true, Ordering::Release);
        info!("dfu: DFU download started");
        publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Started));
        self.inner.start()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Status> {
        publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Downloading));
        self.inner.write(data)
    }

    fn finish(&mut self) -> Result<(), Status> {
        let res = self.inner.finish();
        publish_event(crate::event::DfuStatusEvent::new(if res.is_ok() {
            DfuStatus::Finished
        } else {
            DfuStatus::Error
        }));
        res
    }

    fn system_reset(&mut self) {
        self.inner.system_reset()
    }
}

// ---------------------------------------------------------------------------
// register_dfu_interface
// ---------------------------------------------------------------------------

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use {
    embassy_boot::{BlockingFirmwareUpdater, FirmwareUpdaterConfig},
    embassy_usb_dfu::{ResetImmediate, dfu::FirmwareHandler},
};

/// Register a DFU interface on the USB builder.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn register_dfu_interface<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    mgr: &'static DfuFlashManager,
    product_name: &'static str,
) {
    use embassy_usb::class::dfu::consts::DfuAttributes;

    let dfu_part = mgr.dfu_partition();
    let state_part = mgr.state_partition();
    let config = FirmwareUpdaterConfig {
        dfu: dfu_part,
        state: state_part,
    };
    static ALIGNED: StaticCell<[u8; DFU_WRITE_SIZE]> = StaticCell::new();
    let aligned: &'static mut [u8] = ALIGNED.init([0; DFU_WRITE_SIZE]);
    let updater = BlockingFirmwareUpdater::new(config, aligned);

    let attrs = DfuAttributes::CAN_DOWNLOAD | DfuAttributes::WILL_DETACH;

    let inner = FirmwareHandler::new(updater, ResetImmediate);
    let handler = RmkDfuHandler { inner };
    let state = DfuState::new(handler, attrs);

    type DfuStateInner =
        RmkDfuHandler<FirmwareHandler<'static, PartitionType, PartitionType, ResetImmediate, BLOCK_SIZE_DFU>>;
    static DFU_STATE: StaticCell<DfuState<DfuStateInner>> = StaticCell::new();
    let state_ref = DFU_STATE.init(state);

    let string_idx = builder.string();

    let mut func = builder.function(0x00, 0x00, 0x00);
    let mut iface = func.interface();
    let mut alt = iface.alt_setting(0xFE, 0x01, 0x02, Some(string_idx));
    alt.descriptor(
        0x21,
        &[
            attrs.bits(),
            0xc4,
            0x09,
            (BLOCK_SIZE_DFU & 0xff) as u8,
            ((BLOCK_SIZE_DFU >> 8) & 0xff) as u8,
            0x10,
            0x01,
        ],
    );
    drop(func);
    builder.handler(state_ref);

    static STRING_PROVIDER: StaticCell<DfuStringProvider> = StaticCell::new();
    let string_provider = STRING_PROVIDER.init(DfuStringProvider {
        string_idx,
        string_val: product_name,
    });
    builder.handler(string_provider);
}

// ---------------------------------------------------------------------------
// run_peripheral_dfu
// ---------------------------------------------------------------------------

/// Run a USB DFU-only device on the peripheral side of a split keyboard.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub async fn run_peripheral_dfu<D: Driver<'static>>(
    driver: D,
    device_config: crate::config::DeviceConfig<'static>,
) -> ! {
    use crate::usb::new_usb_builder;

    let mut builder = new_usb_builder(driver, device_config);

    let product_name = device_config.product_name;
    if let Some(mgr) = get_manager() {
        register_dfu_interface(&mut builder, mgr, product_name);
    }

    let mut device = builder.build();

    loop {
        device.run_until_suspend().await;
        device.wait_resume().await;
    }
}

// ---------------------------------------------------------------------------
// dfu_lock
// ---------------------------------------------------------------------------

/// DfuLock state machine that checks a physical key combination to unlock DFU.
#[cfg(feature = "dfu_lock")]
pub struct DfuLock<'a> {
    unlocked: AtomicBool,
    unlock_keys: &'a [(u8, u8)],
    keymap: &'a crate::keymap::KeyMap<'a>,
}

#[cfg(feature = "dfu_lock")]
impl<'a> DfuLock<'a> {
    pub fn new(unlock_keys: &'a [(u8, u8)], keymap: &'a crate::keymap::KeyMap<'a>) -> Self {
        Self {
            unlocked: AtomicBool::new(false),
            unlock_keys,
            keymap,
        }
    }

    pub(crate) async fn process_unlock(&self) {
        DFU_UNLOCK_SIGNAL.wait().await;

        info!("dfu_lock: DFU activity detected, unlock window open for 10 s");
        info!("dfu_lock: waiting for unlock keys");
        publish_event(crate::event::DfuStatusEvent::new(DfuStatus::LockWaiting));
        let deadline = embassy_time::Instant::now() + embassy_time::Duration::from_secs(10);
        loop {
            let all_pressed = self
                .unlock_keys
                .iter()
                .all(|(row, col)| self.keymap.read_matrix_key(*row, *col));
            if all_pressed {
                self.unlocked.store(true, Ordering::Release);
                DFU_LOCKED.store(false, Ordering::Release);
                info!("dfu_lock: unlock keys pressed, DFU unlocked for 10 s");
                publish_event(crate::event::DfuStatusEvent::new(DfuStatus::LockUnlocked));
                break;
            }
            if embassy_time::Instant::now() >= deadline {
                info!("dfu_lock: unlock window expired (10 s timeout)");
                DFU_LOCKED.store(true, Ordering::Release);
                publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Idle));
                return;
            }
            embassy_time::Timer::after_millis(50).await;
        }

        info!("dfu_lock: unlocked, waiting for DFU download");
        let deadline = embassy_time::Instant::now() + embassy_time::Duration::from_secs(10);
        loop {
            if DFU_STARTED.load(Ordering::Acquire) {
                info!("dfu_lock: DFU download started, staying unlocked");
                break;
            }
            if embassy_time::Instant::now() >= deadline {
                info!("dfu_lock: unlock expired (10 s timeout)");
                DFU_LOCKED.store(true, Ordering::Release);
                self.unlocked.store(false, Ordering::Release);
                publish_event(crate::event::DfuStatusEvent::new(DfuStatus::Idle));
                break;
            }
            embassy_time::Timer::after_millis(200).await;
        }
    }
}

#[cfg(feature = "dfu_lock")]
impl<'a> Runnable for DfuLock<'a> {
    async fn run(&mut self) -> ! {
        loop {
            self.process_unlock().await;
        }
    }
}
