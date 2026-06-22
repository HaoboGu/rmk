use core::cell::RefCell;
#[cfg(feature = "dfu_lock")]
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;
#[cfg(feature = "dfu_split")]
use core::sync::atomic::AtomicUsize;

use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::once_lock::OnceLock;
#[cfg(feature = "dfu_lock")]
use embassy_sync::signal::Signal;
use embassy_usb::control::{InResponse, OutResponse, Request};
use embassy_usb::driver::Driver;
use embassy_usb::types::{InterfaceNumber, StringIndex};
use embassy_usb::{Builder, Handler};
use static_cell::StaticCell;

#[cfg(feature = "dfu_lock")]
use crate::core_traits::Runnable;

#[cfg(all(feature = "dfu_split", not(any(feature = "dfu_rp", feature = "dfu_nrf"))))]
compile_error!("dfu_split requires at least one of dfu_rp or dfu_nrf to be enabled");

/// Simple USB string provider for the DFU interface, to show a product name in the host's device manager during DFU mode. The FirmwareHandler of embassy_usb_dfu doesn't use the string index from the interface descriptor, so we have to provide our own handler to return the string when requested by the host.
/// This is the name string that gets shown with `dfu-util -l` option.
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

/// Total flash size passed to the embassy-rp Flash const generic.
///
/// Set to 16 MB (the maximum common RP2040 flash size) so that the same
/// binary works on boards with 2, 4, 8 or 16 MB flash.  `new_blocking()`
/// ignores this value at runtime — it is only used for software bounds
/// checking inside embassy-rp.  Because all flash access goes through
/// `BlockingPartition` (which has its own partition-sized bounds checks),
/// overshooting the const generic is safe.
#[cfg(feature = "dfu_rp")]
pub const FLASH_SIZE: usize = 16 * 1024 * 1024;

/// DFU transfer block size in bytes. Larger values speed up firmware
/// downloads. Must match the USB control buffer size used by the host.
pub const BLOCK_SIZE_DFU: usize = 512;

/// Flash write granularity — 1 for RP2040, 4 for nRF NVMC.
#[cfg(feature = "dfu_rp")]
const DFU_WRITE_SIZE: usize = 1;
#[cfg(feature = "dfu_nrf")]
const DFU_WRITE_SIZE: usize = 4;

#[cfg(feature = "dfu_nrf")]
use embassy_nrf::{Peri, gpio::Output, nvmc::Nvmc, peripherals::NVMC};
#[cfg(feature = "dfu_rp")]
use embassy_rp::{
    Peri,
    flash::{Blocking, Flash},
    gpio::Output,
    peripherals::FLASH,
};
#[cfg(feature = "dfu")]
use embassy_usb::class::dfu::{
    consts::Status,
    dfu_mode::{self, DfuState},
};
#[cfg(feature = "dfu")]
use embassy_usb_dfu::{ResetImmediate, dfu::FirmwareHandler};
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use {
    embassy_boot::BlockingFirmwareState,
    embassy_embedded_hal::flash::partition::BlockingPartition,
};

#[cfg(feature = "dfu_rp")]
type FlashType = Flash<'static, FLASH, Blocking, FLASH_SIZE>;
#[cfg(feature = "dfu_nrf")]
type FlashType = Nvmc<'static>;

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
type MutexType = Mutex<CriticalSectionRawMutex, RefCell<FlashType>>;
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
type PartitionType = BlockingPartition<'static, CriticalSectionRawMutex, FlashType>;

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
    fn new(
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

// Per-chip statics
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
static FLASH_CELL: StaticCell<MutexType> = StaticCell::new();
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
static MANAGER: OnceLock<DfuFlashManager> = OnceLock::new();
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
static LED: OnceLock<Mutex<CriticalSectionRawMutex, RefCell<Option<Output<'static>>>>> = OnceLock::new();

/// Store an optional DFU LED pin globally.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn set_led(led: Option<Output<'static>>) {
    let _ = LED.init(Mutex::new(RefCell::new(led)));
}

/// Run a closure with the global DFU LED, if configured (platform-specific impl).
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub(crate) fn with_led<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Output<'static>) -> R,
{
    LED.try_get()
        .and_then(|m| m.lock(|cell| cell.borrow_mut().as_mut().map(f)))
}

/// No-op fallback when DFU is built without an LED-capable platform driver.
#[cfg(all(feature = "dfu", not(any(feature = "dfu_rp", feature = "dfu_nrf"))))]
pub(crate) fn with_led<F, R>(_: F) -> Option<R> {
    None
}

/// DFU lock state (shared between keyboard loop and DFU handler).
#[cfg(feature = "dfu_lock")]
static DFU_LOCKED: AtomicBool = AtomicBool::new(true);

/// Set to true once `RmkDfuHandler::start()` begins a DFU download.
#[cfg(feature = "dfu_lock")]
static DFU_STARTED: AtomicBool = AtomicBool::new(false);

/// Signaled by `RmkDfuHandler::start()` when a blocked DFU download is rejected.
/// Triggers the unlock key polling window in `process_unlock`.
#[cfg(feature = "dfu_lock")]
static DFU_UNLOCK_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[cfg(feature = "dfu_lock")]
pub fn is_dfu_unlocked() -> bool {
    !DFU_LOCKED.load(Ordering::Acquire)
}

/// DFU handler wrapper that blinks an LED during transfer and checks the
/// DFU lock (if `dfu_lock` feature is enabled).
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
        with_led(|led| led.set_high());
        self.inner.start()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Status> {
        with_led(|led| led.toggle());
        self.inner.write(data)
    }

    fn finish(&mut self) -> Result<(), Status> {
        let res = self.inner.finish();
        with_led(|led| led.set_low());
        res
    }

    fn system_reset(&mut self) {
        with_led(|led| led.set_low());
        self.inner.system_reset()
    }
}

#[cfg(feature = "dfu_rp")]
type FlashPeri = Peri<'static, FLASH>;
#[cfg(feature = "dfu_nrf")]
type FlashPeri = Peri<'static, NVMC>;

/// Initialize the blocking flash, create the DFU manager and store it globally.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn init_flash(
    flash_peri: FlashPeri,
    storage_offset: u32,
    storage_size: u32,
    state_offset: u32,
    state_size: u32,
    dfu_offset: u32,
    dfu_size: u32,
) -> PartitionType {
    #[cfg(feature = "dfu_nrf")]
    let raw_flash = Nvmc::new(flash_peri);
    #[cfg(feature = "dfu_rp")]
    let raw_flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(flash_peri);

    let flash_mutex: &'static MutexType = FLASH_CELL.init(Mutex::new(RefCell::new(raw_flash)));
    let mgr = DfuFlashManager::new(
        flash_mutex,
        storage_offset,
        storage_size,
        state_offset,
        state_size,
        dfu_offset,
        dfu_size,
    );
    let partition = mgr.storage_partition();
    MANAGER.init(mgr).ok();
    partition
}

/// Mark firmware boot as successful so the bootloader doesn't revert on next reset.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn mark_booted() {
    if let Some(mgr) = get_manager() {
        let state_part = mgr.state_partition();
        static ALIGNED: StaticCell<[u8; DFU_WRITE_SIZE]> = StaticCell::new();
        let aligned: &'static mut [u8] = ALIGNED.init([0; DFU_WRITE_SIZE]);
        let mut state = BlockingFirmwareState::new(state_part, aligned);
        state.mark_booted().ok();
    }
}

/// Get a reference to the global DFU flash manager.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn get_manager() -> Option<&'static DfuFlashManager> {
    MANAGER.try_get()
}

/// Max passthrough alt settings supported on a single DFU interface.
#[cfg(feature = "dfu_split")]
const MAX_PASSTHROUGH_ALTS: usize = 4;

/// Single USB `Handler` that owns **all** DFU alternate settings.
///
/// # Alt-setting routing
///
/// Without `dfu_split` this is a thin wrapper around the central `DfuState`
/// (Alt 0).  With `dfu_split` it additionally holds one passthrough
/// `DfuState` per split peripheral (Alt 1 … N) and dispatches USB control
/// requests to the correct sub-handler based on the current alternate
/// setting, tracked via [`Handler::set_alternate_setting`].
///
/// # Flow control for passthrough
///
/// The DFU host polls GETSTATUS after every DNLOAD and waits for the
/// device to leave `dfuDNBUSY` before sending the next block.  We exploit
/// this: when the PeripheralManager is still forwarding the *previous*
/// chunk (indicated by `PASSTHROUGH_TARGET != MAX`), `control_in` overrides
/// the GETSTATUS response to report `dfuDNBUSY`.  As soon as forwarding
/// completes the host sees the real state and sends the next block
/// immediately.  This lets the passthrough work at line-speed regardless
/// of firmware or transport (UART, BLE, …) — no fixed timeouts, no
/// huge queues.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
struct RmkDfuInterface {
    central: DfuState<
        RmkDfuHandler<
            FirmwareHandler<'static, PartitionType, PartitionType, ResetImmediate, BLOCK_SIZE_DFU>,
        >,
    >,
    #[cfg(feature = "dfu_split")]
    passthrough: [Option<DfuState<RmkDfuHandler<PassthroughDfuHandler>>>; MAX_PASSTHROUGH_ALTS],
    #[cfg(feature = "dfu_split")]
    num_passthrough: usize,
    current_alt: u8,
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
impl Handler for RmkDfuInterface {
    /// Called by embassy-usb after the host selects an alternate setting.
    ///
    /// Alt 0 → central's own DFU flash.  Alt 1..N → passthrough to a
    /// split peripheral (only available when `dfu_split` is enabled).
    /// The value is read by `control_out` and `control_in` to dispatch
    /// USB requests to the right sub-handler.
    fn set_alternate_setting(&mut self, _iface: InterfaceNumber, alternate_setting: u8) {
        info!("dfu: set_alternate_setting(iface={}, alt={})", _iface.0, alternate_setting);
        self.current_alt = alternate_setting;
    }

    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        match self.current_alt {
            0 => self.central.control_out(req, data),
            #[cfg(feature = "dfu_split")]
            n => {
                let idx = (n as usize).wrapping_sub(1);
                self.passthrough_slots(idx)
                    .and_then(|s| s.control_out(req, data))
            }
            #[cfg(not(feature = "dfu_split"))]
            _ => None,
        }
    }

    /// Dispatch control-IN requests to the active alternate setting.
    ///
    /// Alt 0 is forwarded to the central `DfuState` (normal DFU boots).
    ///
    /// For passthrough alts (1..N) this method:
    ///
    /// 1. Forwards the request to the passthrough `DfuState` so it can
    ///    generate the standard response (e.g. `GETSTATUS`, `GETSTATE`).
    /// 2. **After** the sub-handler writes its response, inspects
    ///    [`PASSTHROUGH_TARGET`] to decide whether the PeripheralManager
    ///    has finished processing the previous chunk.
    /// 3. If the target is still set (chunk not yet forwarded), overrides
    ///    the `state` byte in the GETSTATUS response to `dfuDNBUSY` (4).
    ///    The host sees "device busy" and polls again after 50 ms.
    /// 4. Once the target becomes `usize::MAX` the real DFU state is
    ///    returned and the host immediately sends the next DNLOAD.
    ///
    /// This provides **adaptive flow control**: no fixed timeouts, no
    /// large queues, no spinning in the USB ISR.
    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        match self.current_alt {
            0 => self.central.control_in(req, buf),
            #[cfg(feature = "dfu_split")]
            n => {
                let idx = (n as usize).wrapping_sub(1);
                // Obtain raw pointer before buf is borrowed by the sub-handler.
                let buf_ptr = buf.as_mut_ptr();
                let resp = self.passthrough_slots(idx)
                    .and_then(|s| s.control_in(req, buf));
                // Flow control for passthrough: if the PeripheralManager is
                // still forwarding the previous chunk, tell the host we are
                // busy so it polls again instead of sending the next block.
                //
                // GETSTATUS response layout (6 bytes):
                //   [0]=status  [1..4]=bwPollTimeout  [4]=state  [5]=iString
                // We override byte 4 from whatever DfuState returned to
                // dfuDNBUSY (4).  The host will poll again every 50 ms until
                // the PeripheralManager finishes and `PASSTHROUGH_TARGET`
                // becomes `usize::MAX`.
                //
                // Safety: `buf_ptr` was captured before the `InResponse`
                // reference was created.  The write goes through a volatile
                // store so the compiler does not reorder or elide it.
                if resp.is_some() && PASSTHROUGH_TARGET.load(Ordering::Acquire) != usize::MAX {
                    unsafe { core::ptr::write_volatile(buf_ptr.add(4), 4u8); }
                }
                resp
            }
            #[cfg(not(feature = "dfu_split"))]
            _ => None,
        }
    }
}

#[cfg(feature = "dfu_split")]
impl RmkDfuInterface {
    fn passthrough_slots(
        &mut self,
        idx: usize,
    ) -> Option<&mut DfuState<RmkDfuHandler<PassthroughDfuHandler>>> {
        self.passthrough.get_mut(idx)?.as_mut()
    }
}

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
static RMK_DFU_INTERFACE: StaticCell<RmkDfuInterface> = StaticCell::new();

/// Register a DFU interface on the USB builder.
///
/// Alt 0 is always the central's own DFU flash (for `dfu-util -D central.bin -R`).
/// Alts 1..N are passthrough slots for split peripherals (requires `dfu_split`).
/// Pass `num_peripherals = 0` (or omit via cfg) when no passthrough is needed.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn register_dfu_interface<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    mgr: &'static DfuFlashManager,
    product_name: &'static str,
    #[cfg(feature = "dfu_split")] num_peripherals: usize,
) {
    use embassy_boot_rp::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
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

    // ── Alt 0: Central flash ──
    let central_attrs = DfuAttributes::CAN_DOWNLOAD | DfuAttributes::WILL_DETACH;

    let inner = FirmwareHandler::new(updater, ResetImmediate);
    let central_handler = RmkDfuHandler { inner };
    let central_state = DfuState::new(central_handler, central_attrs);

    // ── Alt 1..N: Passthrough ──
    #[cfg(feature = "dfu_split")]
    let passthrough_count = num_peripherals.min(MAX_PASSTHROUGH_ALTS);

    #[cfg(feature = "dfu_split")]
    let passthrough = {
        let mut arr: [Option<DfuState<RmkDfuHandler<PassthroughDfuHandler>>>; MAX_PASSTHROUGH_ALTS] =
            Default::default();
        for id in 0..passthrough_count {
            let state = DfuState::new(
                RmkDfuHandler {
                    inner: PassthroughDfuHandler {
                        target_id: id,
                        written: 0,
                    },
                },
                DfuAttributes::CAN_DOWNLOAD,
            );
            arr[id] = Some(state);
        }
        arr
    };

    let string_idx = builder.string();

    // ── Build descriptors ──
    let mut func = builder.function(0x00, 0x00, 0x00);
    let mut iface = func.interface();
    let mut alt = iface.alt_setting(0xFE, 0x01, 0x02, Some(string_idx)); // class-specific DFU interface with string descriptor for product name
    alt.descriptor(
        0x21, // DFU functional descriptor
        &[
            central_attrs.bits(),
            0xc4, // detach timeout in ms (09c4 = 2500 ms)
            0x09,
            (BLOCK_SIZE_DFU & 0xff) as u8,        // transfer size low byte
            ((BLOCK_SIZE_DFU >> 8) & 0xff) as u8, // transfer size high byte
            0x10,                                 // DFU version 1.1 (BCD 0x0110)
            0x01,
        ],
    );

    #[cfg(feature = "dfu_split")]
    for _ in 0..passthrough_count {
        let mut alt = iface.alt_setting(0xFE, 0x01, 0x02, Some(string_idx));
        alt.descriptor(
            0x21,
            &[
                DfuAttributes::CAN_DOWNLOAD.bits(),
                0xc4,
                0x09,
                (BLOCK_SIZE_DFU & 0xff) as u8,
                ((BLOCK_SIZE_DFU >> 8) & 0xff) as u8,
                0x10,
                0x01,
            ],
        );
    }

    drop(func);

    // ── Register single handler that routes by alt setting ──
    let iface_ref = RMK_DFU_INTERFACE.init(RmkDfuInterface {
        central: central_state,
        #[cfg(feature = "dfu_split")]
        passthrough,
        #[cfg(feature = "dfu_split")]
        num_passthrough: passthrough_count,
        current_alt: 0,
    });
    builder.handler(iface_ref);

    static STRING_PROVIDER: StaticCell<DfuStringProvider> = StaticCell::new();
    let string_provider = STRING_PROVIDER.init(DfuStringProvider {
        string_idx,
        string_val: product_name,
    });
    builder.handler(string_provider);
}

// ---------------------------------------------------------------------------
// dfu_split — split peripheral firmware update over UART
// ---------------------------------------------------------------------------

/// Simple DFU handler for split-peripheral firmware updates.
///
/// Created on-demand via [`SplitDfuHandler::new`]; no global singleton needed.
/// Each method call clones the partition handles and locks the flash mutex
/// internally via `BlockingPartition`.
#[cfg(feature = "dfu_split")]
pub struct SplitDfuHandler {
    dfu_partition: PartitionType,
    state_partition: PartitionType,
    erased: bool,
    written_len: u32,
}

#[cfg(feature = "dfu_split")]
impl SplitDfuHandler {
    /// Create a new handler from the global [`DfuFlashManager`].
    /// Returns `None` if `init_flash` has not been called yet.
    pub fn new() -> Option<Self> {
        let mgr = get_manager()?;
        Some(Self {
            dfu_partition: mgr.dfu_partition(),
            state_partition: mgr.state_partition(),
            erased: false,
            written_len: 0,
        })
    }

    /// Write a chunk of firmware data at the given partition offset.
    ///
    /// On the first call the **entire** DFU partition is erased once;
    /// subsequent calls only write.
    pub fn write_chunk(&mut self, offset: u32, data: &[u8]) -> Result<(), ()> {
        use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
        let mut dfu = self.dfu_partition.clone();

        if !self.erased {
            let cap = ReadNorFlash::capacity(&dfu) as u32;
            dfu.erase(0, cap).map_err(|_| ())?;
            self.erased = true;
        }

        dfu.write(offset, data).map_err(|_| ())?;
        self.written_len = self.written_len.max(offset + data.len() as u32);
        with_led(|led| led.toggle());
        Ok(())
    }

    /// Read back the entire DFU partition and compute its CRC-32.
    /// Used by the peripheral for end-to-end verification before resetting.
    pub fn compute_dfu_crc(&self) -> u32 {
        use embedded_storage::nor_flash::ReadNorFlash;
        let mut dfu = self.dfu_partition.clone();
        let len = self.written_len as usize;
        let mut crc = crate::crc32::Crc32::new();
        let mut buf = [0u8; 256];
        let mut pos = 0u32;
        while (pos as usize) < len {
            let chunk_len = core::cmp::min(256, len - pos as usize);
            dfu.read(pos, &mut buf[..chunk_len]).ok();
            crc.update(&buf[..chunk_len]);
            pos += chunk_len as u32;
        }
        crc.finalize()
    }

    /// Mark firmware as valid and reset into the new image.
    pub fn mark_updated_and_reset(&self) -> Result<(), ()> {
        with_led(|led| led.set_high());
        use embassy_boot_rp::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
        let config = FirmwareUpdaterConfig {
            dfu: self.dfu_partition.clone(),
            state: self.state_partition.clone(),
        };
        static ALIGNED: StaticCell<[u8; 1]> = StaticCell::new();
        let mut updater = BlockingFirmwareUpdater::new(config, ALIGNED.init([0; 1]));
        updater.mark_updated().map_err(|_| ())?;
        cortex_m::peripheral::SCB::sys_reset()
    }
}

// ---------------------------------------------------------------------------
// dfu_split — firmware update data for split peripheral firmware update
// ---------------------------------------------------------------------------

/// Firmware update data for a single split peripheral.
#[cfg(feature = "dfu_split")]
struct FirmwareSlot {
    data: &'static [u8],
    hash: u32,
}

#[cfg(feature = "dfu_split")]
const MAX_FW_SLOTS: usize = 8;

/// Global registry mapping peripheral IDs to their update firmware.
#[cfg(feature = "dfu_split")]
static FW_SLOTS: Mutex<CriticalSectionRawMutex, RefCell<heapless::Vec<(usize, FirmwareSlot), MAX_FW_SLOTS>>> =
    Mutex::new(RefCell::new(heapless::Vec::new()));

/// Store a reference to the peripheral firmware binary and its CRC32 hash.
///
/// The central calls this before starting the split peripheral manager so
/// that `PeripheralManager` can verify and update the peripheral's firmware
/// at connection time.  The `id` must match the peripheral index used in
/// `[[split.peripheral]]` (or the `id` argument of `run_peripheral_manager`).
///
/// Returns `Err(())` if the registry is full (max `MAX_FW_SLOTS` entries).
#[cfg(feature = "dfu_split")]
pub fn set_firmware_update_data(id: usize, firmware: &'static [u8], hash: u32) -> Result<(), ()> {
    FW_SLOTS.lock(|cell| {
        let slots = &mut cell.borrow_mut();
        if let Some(slot) = slots.iter_mut().find(|(i, _)| *i == id) {
            *slot = (id, FirmwareSlot { data: firmware, hash });
        } else {
            slots.push((id, FirmwareSlot { data: firmware, hash })).map_err(|_| ())?;
        }
        Ok(())
    })
}

/// Retrieve the stored peripheral firmware data for a given peripheral ID, if any.
#[cfg(feature = "dfu_split")]
pub fn get_firmware_update_data(id: usize) -> Option<(&'static [u8], u32)> {
    FW_SLOTS.lock(|cell| {
        let slots = cell.borrow();
        slots.iter().find(|(i, _)| *i == id).map(|(_, s)| (s.data, s.hash))
    })
}

// ---------------------------------------------------------------------------
// dfu_split passthrough — forward DFU download to split peripheral in real time
// ---------------------------------------------------------------------------
//
// Architecture
// ============
//
//  Host (dfu-util)                Central (RMK)               Peripheral
//  ═══════════════                ══════════════               ══════════
//  USB DNLOAD(block) ────────► DFU handler (ISR)
//                                  │ store chunk in queue
//                                  │ return immediately
//                                  ▼
//  USB GETSTATUS ◄───────────── RmkDfuInterface (ISR)
//                                  │ if chunk still pending → dfuDNBUSY
//                                  │ if chunk done  → Download
//                                  ▼
//                            PeripheralManager (async task)
//                                  │ take chunk from queue
//                                  │ FirmwareChunk over UART ──────► SplitPeripheral
//                                  │                                   │ write flash
//                                  │ ◄──── FirmwareChunkAck ───────── return
//                                  │ signal done (TARGET = MAX)
//
// The key insight is that we **never spin in the USB ISR**.  Chunks are
// stored in a small fire-and-forget queue and the ISR returns immediately.
// The PeripheralManager (an async task running on the embassy executor)
// forwards them over the split link.  GETSTATUS flow-control gives the
// executor enough time to process each chunk before the host sends the
// next one.
//
// Protocol (embedded firmware update)
// ====================================
//
// ═══ PHASE 0: HANDSHAKE ── hash comparison ─────────────────────────
//
//   Central                           Peripheral
//     │                                   │
//     ├── FirmwareHashQuery ─────────────>│
//     │<── FirmwareHashResponse(hash) ────┤
//     │       (or announced on connect)   │
//     │                                   │
//     │  hash == expected_hash?           │
//     │    ├─ Yes → STOP (up-to-date)     │
//     │    └─ No  → ⬇                     │
//
//
// ═══ PHASE 1: CHUNK TRANSFER ── per-chunk CRC ─────────────────────
//   outer attempt loop: 1..3
//
//   Central                           Peripheral
//     │                                   │
//     │ LED ON, central_crc = Crc32::new()
//     │   for chunk in firmware[256]:
//     │     chunk_crc = CRC32(chunk)
//     │     central_crc.update(chunk)
//     │   retry = 0
//     │   ┌─ retry < 3 ─────────────────┐
//     │   │                             │
//     ├── FirmwareChunk{offset,data} ──>│
//     │   │             handler.write_chunk() — erase DFU on 1st call
//     │   │             CRC32(chunk)          │
//     │   │<─ FirmwareChunkAck{offset,crc} ───┤
//     │   │                             │
//     │   ack_crc == chunk_crc?         │
//     │     ├─ Yes → next chunk         │
//     │     └─ No  → retry++ ──────────┘
//     │   All chunks acked?
//     │     ├─ Yes → ⬇
//     │     └─ No  → outer attempt++
//
//
// ═══ PHASE 2: END-TO-END CRC ── flash readback ────────────────────
//
//   Central                           Peripheral
//     │                                   │
//     │ central_crc.finalize()
//     │ == expected_hash?
//     │   ├─ No  → ABORT (binary bug!)
//     │   └─ Yes → ⬇
//     │                                   │
//     ├── FirmwareUpdateComplete ────────>│
//     │                     handler.compute_dfu_crc()
//     │                     = CRC32(whole DFU partition)
//     │<── FirmwareCrcReport(dfu_crc) ────┤
//     │                                   │
//     │  dfu_crc == expected_hash?        │
//     │    ├─ Yes → FirmwareCrcOk ──────>│
//     │    │                handler.mark_updated_and_reset()
//     │    │<─ FirmwareUpdateConfirm ─────┤  DONE
//     │    └─ No  → FirmwareCrcFail ────>│  outer attempt++
//     │                                   │
//
// ═══ RETRY SUMMARY ═════════════════════════════════════════════════
//
//   Layer           Max   Trigger              Consequence
//   ─────           ───   ───────              ──────────
//   Per-chunk       3×    Ack CRC mismatch     Re-send same chunk
//                         or 2s timeout
//   Outer attempt   3×    Chunk never acked    Full restart of
//                         or E2E CRC mismatch  Phase 1 + 2
//
// ═══ SAFETY GATES ═════════════════════════════════════════════════
//
//   mark_updated only on FirmwareCrcOk   Never boot into corrupt FW
//   central_crc == expected_hash         Catch binary bugs
//   compute_dfu_crc() flash readback     Catch silent flash write errors
//   Per-chunk CRC in Ack                 Catch packet loss / bitflips

/// A single firmware chunk received from the host via DFU, destined for a
/// split peripheral.
#[cfg(feature = "dfu_split")]
pub(crate) struct PassthroughChunk {
    pub offset: u32,
    pub data: [u8; 256],
    pub len: u16,
}

/// Commands flowing from the DFU handler (synchronous USB ISR) to the
/// PeripheralManager (async embassy task).
#[cfg(feature = "dfu_split")]
pub(crate) enum PassthroughCommand {
    Chunk(PassthroughChunk),
    Finish,
}

/// Small fire-and-forget queue.  With GETSTATUS flow-control the actual
/// backlog rarely exceeds 2–3 chunks, so 4 slots provide ample margin.
#[cfg(feature = "dfu_split")]
const PASSTHROUGH_QUEUE_SIZE: usize = 4;

/// Single-producer / single-consumer command queue protected by a critical-
/// section mutex.  The DFU handler (ISR) pushes, the PeripheralManager pops.
#[cfg(feature = "dfu_split")]
static PASSTHROUGH_CMD: Mutex<CriticalSectionRawMutex, RefCell<heapless::Vec<PassthroughCommand, PASSTHROUGH_QUEUE_SIZE>>> =
    Mutex::new(RefCell::new(heapless::Vec::new()));

/// Signals which peripheral has a pending passthrough command.
///
/// Set to the peripheral `id` by the DFU handler when a chunk or Finish
/// command is queued.  Cleared to `usize::MAX` by the
/// [`passthrough_done_if_empty`] helper once **all** pending commands for
/// that peripheral have been consumed.
///
/// # Flow-control integration
///
/// [`RmkDfuInterface::control_in`] reads this atomic on every GETSTATUS
/// request while a passthrough alternate setting is active.  If the value
/// differs from `usize::MAX` the GETSTATUS response is overridden to
/// `dfuDNBUSY`, instructing the host to poll again after 50 ms instead of
/// sending the next DNLOAD block.  This gives the async executor (running
/// the `PeripheralManager`) enough CPU time to forward the previous chunk
/// over the split link.
///
/// The flow is therefore **adaptive**: the host automatically waits when
/// the split link is congested, and proceeds at line-speed when it isn't.
/// No fixed timeouts, no large RAM queues.
#[cfg(feature = "dfu_split")]
pub(crate) static PASSTHROUGH_TARGET: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Check whether a passthrough command is pending for the given peripheral id.
#[cfg(feature = "dfu_split")]
pub(crate) fn passthrough_pending(id: usize) -> bool {
    PASSTHROUGH_TARGET.load(Ordering::Acquire) == id
}


/// DFU handler for passthrough — slices USB DNLOAD data into 256 byte
/// chunks, pushes them into the queue, and returns immediately.
///
/// **Never blocks or spin-waits.**  This runs inside the USB control-request
/// ISR, so any delay here would starve the embassy executor and prevent the
/// PeripheralManager from draining the queue.  All forwarding happens
/// asynchronously in the PeripheralManager task.
///
/// GETSTATUS flow-control (see [`RmkDfuInterface`]) ensures the host waits
/// before sending the next block, giving the executor enough time to forward
/// each chunk to the peripheral.
#[cfg(feature = "dfu_split")]
struct PassthroughDfuHandler {
    target_id: usize,
    /// Total bytes written so far, used as the flash offset on the peripheral.
    written: u32,
}

/// Push a command into the fire-and-forget queue.
///
/// Called from the DFU handler ISR — **never blocks or spins**.  Returns
/// `Err` if the queue is full (should be rare because GETSTATUS flow
/// control prevents the host from sending the next block while the queue
/// is draining).
#[cfg(feature = "dfu_split")]
fn passthrough_push(cmd: PassthroughCommand) -> Result<(), ()> {
    PASSTHROUGH_CMD.lock(|c| c.borrow_mut().push(cmd).map_err(|_| ()))
}

/// Take the next pending command from the queue (FIFO).
///
/// Called by [`PeripheralManager::handle_passthrough`] in the async
/// executor.  Returns `None` when the queue is empty — the caller then
/// calls [`passthrough_done_if_empty`] to release the TARGET for the
/// next host polling cycle.
#[cfg(feature = "dfu_split")]
pub(crate) fn passthrough_take_command() -> Option<PassthroughCommand> {
    PASSTHROUGH_CMD.lock(|c| {
        let v = &mut *c.borrow_mut();
        if !v.is_empty() { Some(v.remove(0)) } else { None }
    })
}

/// Release [`PASSTHROUGH_TARGET`] when the queue is empty.
///
/// Called after every command is fully processed.  If the queue still has
/// pending commands the target is **not** released, so
/// [`RmkDfuInterface::control_in`] continues to report `dfuDNBUSY` and
/// the host keeps polling.  Once empty, the target is reset to
/// `usize::MAX` and the host sees the real DFU state on its next
/// GETSTATUS poll.
#[cfg(feature = "dfu_split")]
pub(crate) fn passthrough_done_if_empty() {
    let empty = PASSTHROUGH_CMD.lock(|c| c.borrow().is_empty());
    if empty {
        PASSTHROUGH_TARGET.store(usize::MAX, Ordering::Release);
    }
}

#[cfg(feature = "dfu_split")]
impl dfu_mode::Handler for PassthroughDfuHandler {
    /// Called by DfuState when the host sends the first DNLOAD block
    /// (block number 0).  Resets the write cursor.
    fn start(&mut self) -> Result<(), Status> {
        self.written = 0;
        Ok(())
    }

    /// Called by DfuState for every subsequent DNLOAD block.
    ///
    /// Slices the incoming 512‑byte USB block into 256‑byte chunks and
    /// pushes them into the fire-and-forget queue.  **Never blocks or
    /// spins** — the ISR returns immediately and the `PeripheralManager`
    /// forwards the chunks asynchronously.
    ///
    /// GETSTATUS flow control (see [`PASSTHROUGH_TARGET`]) ensures the
    /// host does not send the next DNLOAD until the queue is drained.
    fn write(&mut self, data: &[u8]) -> Result<(), Status> {
        for chunk in data.chunks(256) {
            let mut buf = [0u8; 256];
            buf[..chunk.len()].copy_from_slice(chunk);
            if passthrough_push(PassthroughCommand::Chunk(PassthroughChunk {
                offset: self.written,
                len: chunk.len() as u16,
                data: buf,
            })).is_err() {
                error!("passthrough queue full");
                return Err(Status::ErrUnknown);
            }
            self.written += chunk.len() as u32;
        }
        PASSTHROUGH_TARGET.store(self.target_id, Ordering::Release);
        Ok(())
    }

    /// Called by DfuState when the host signals end-of-transfer
    /// (DNLOAD with `wLength = 0`).
    ///
    /// Pushes a [`PassthroughCommand::Finish`] into the queue.  The
    /// `PeripheralManager` picks it up, asks the peripheral to verify
    /// the DFU partition CRC, and confirms the update.
    fn finish(&mut self) -> Result<(), Status> {
        if passthrough_push(PassthroughCommand::Finish).is_err() {
            error!("passthrough queue full at finish");
            return Err(Status::ErrUnknown);
        }
        PASSTHROUGH_TARGET.store(self.target_id, Ordering::Release);
        Ok(())
    }

    /// No-op — the peripheral resets itself via `mark_updated_and_reset()`
    /// after a successful update.  The central's USB passthrough handler
    /// must not reset the central.
    fn system_reset(&mut self) {}
}

/// Return the CRC32 of this device's currently running firmware binary.
///
/// Reads the ACTIVE flash partition from `__vector_table` to `__veneer_limit`
/// and computes CRC32 on the fly.  The result is cached in a static so that
/// subsequent calls are instant.
#[cfg(feature = "dfu_split")]
pub fn read_embedded_firmware_hash() -> u32 {
    use core::sync::atomic::AtomicU32;

    static CACHED_HASH: AtomicU32 = AtomicU32::new(0);

    let cached = CACHED_HASH.load(Ordering::Acquire);
    if cached != 0 {
        return cached;
    }

    unsafe extern "C" {
        static __vector_table: u8;
        static __veneer_limit: u8;
    }

    let start = unsafe { &__vector_table as *const u8 };
    let end = unsafe { &__veneer_limit as *const u8 };
    let len = end as usize - start as usize;
    let data = unsafe { core::slice::from_raw_parts(start, len) };
    let hash = crate::crc32::crc32(data);

    CACHED_HASH.store(hash, Ordering::Release);
    hash
}

// ---------------------------------------------------------------------------
// dfu_lock — physical key unlock for DFU firmware download
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

    /// One unlock cycle: block (yielded) on `DFU_UNLOCK_SIGNAL.wait()` until a
    /// DFU download is rejected, then open a 10 s unlock window polling the
    /// configured unlock keys at 50 ms. If the keys are pressed within that
    /// window the lock is released and the LED blinks Morse "D F U".
    pub(crate) async fn process_unlock(&self) {
        // Phase 1: wait (yielded) until a DFU unlock request arrives
        DFU_UNLOCK_SIGNAL.wait().await;

        // Phase 2: start the 10 s unlock window, wait for keypress or timeout.
        // LED solid on to signal "press unlock keys now".
        info!("dfu_lock: DFU activity detected, unlock window open for 10 s");
        info!("dfu_lock: waiting for unlock keys");
        with_led(|led| led.set_high());
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
                break;
            }
            if embassy_time::Instant::now() >= deadline {
                info!("dfu_lock: unlock window expired (10 s timeout)");
                DFU_LOCKED.store(true, Ordering::Release);
                with_led(|led| led.set_low());
                return;
            }
            embassy_time::Timer::after_millis(50).await;
        }

        // Phase 3: unlocked — wait until DFU download starts or 10 s pass.
        // LED blinks Morse "D F U" to signal "ready to flash".
        info!("dfu_lock: unlocked, signalling with Morse DFU");
        let deadline = embassy_time::Instant::now() + embassy_time::Duration::from_secs(10);
        loop {
            if DFU_STARTED.load(Ordering::Acquire) {
                info!("dfu_lock: DFU download started, staying unlocked");
                with_led(|led| led.set_high());
                break;
            }
            if embassy_time::Instant::now() >= deadline {
                info!("dfu_lock: unlock expired (10 s timeout)");
                DFU_LOCKED.store(true, Ordering::Release);
                self.unlocked.store(false, Ordering::Release);
                with_led(|led| led.set_low());
                break;
            }
            Self::morse_blink_dfu().await;
        }
    }

    /// Blink "D F U" in Morse code on the DFU LED.
    async fn morse_blink_dfu() {
        use embassy_time::Timer;

        /// Element timing in milliseconds
        const DOT: u64 = 100;
        const DASH: u64 = 300;
        const PAUSE_ELEMENT: u64 = 100;
        const PAUSE_LETTER: u64 = 300;

        macro_rules! dot {
            () => {
                with_led(|led| led.set_high());
                Timer::after_millis(DOT).await;
                with_led(|led| led.set_low());
                Timer::after_millis(PAUSE_ELEMENT).await;
            };
        }
        macro_rules! dash {
            () => {
                with_led(|led| led.set_high());
                Timer::after_millis(DASH).await;
                with_led(|led| led.set_low());
                Timer::after_millis(PAUSE_ELEMENT).await;
            };
        }
        macro_rules! letter_gap {
            () => {
                Timer::after_millis(PAUSE_LETTER - PAUSE_ELEMENT).await;
            };
        }

        // D = -..
        dash!();
        dot!();
        dot!();
        letter_gap!();
        // F = ..-.
        dot!();
        dot!();
        dash!();
        dot!();
        letter_gap!();
        // U = ..-
        dot!();
        dot!();
        dash!();
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
