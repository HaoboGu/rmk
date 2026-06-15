use core::cell::RefCell;
#[cfg(feature = "dfu_lock")]
use core::sync::atomic::AtomicBool;
#[cfg(feature = "dfu_split")]
use core::sync::atomic::AtomicU32;
use core::sync::atomic::{AtomicPtr, Ordering};

use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::once_lock::OnceLock;
#[cfg(feature = "dfu_lock")]
use embassy_sync::signal::Signal;
use embassy_usb::control::{InResponse, OutResponse, Request};
use embassy_usb::driver::Driver;
use embassy_usb::types::StringIndex;
use embassy_usb::{Builder, Handler};
use static_cell::StaticCell;

#[cfg(feature = "dfu_lock")]
use crate::core_traits::Runnable;

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
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use {
    embassy_boot::{BlockingFirmwareState, BlockingFirmwareUpdater, FirmwareUpdaterConfig},
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
fn with_led<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Output<'static>) -> R,
{
    LED.try_get()
        .and_then(|m| m.lock(|cell| cell.borrow_mut().as_mut().map(f)))
}

/// No-op fallback when DFU is built without an LED-capable platform driver.
#[cfg(all(feature = "dfu", not(any(feature = "dfu_rp", feature = "dfu_nrf"))))]
fn with_led<F, R>(_: F) -> Option<R> {
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

/// Register a DFU interface on the USB builder.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
pub fn register_dfu_interface<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    mgr: &'static DfuFlashManager,
    product_name: &'static str,
) {
    use embassy_boot_rp::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
    use embassy_usb::class::dfu::consts::DfuAttributes;
    use embassy_usb_dfu::ResetImmediate;
    use embassy_usb_dfu::dfu::FirmwareHandler;

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
    let mut alt = iface.alt_setting(0xFE, 0x01, 0x02, Some(string_idx)); // class-specific DFU interface with string descriptor for product name
    alt.descriptor(
        0x21, // DFU functional descriptor
        &[
            attrs.bits(),
            0xc4, // detach timeout in ms (09c4 = 2500 ms)
            0x09,
            (BLOCK_SIZE_DFU & 0xff) as u8,        // transfer size low byte
            ((BLOCK_SIZE_DFU >> 8) & 0xff) as u8, // transfer size high byte
            0x10,                                 // DFU version 1.1 (BCD 0x0110)
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
        })
    }

    /// Write a chunk of firmware data at the given partition offset.
    ///
    /// On the first call the **entire** DFU partition is erased once;
    /// subsequent calls only write.  This matches the split protocol
    /// which sends sequential chunks from offset 0.
    pub fn write_chunk(&mut self, offset: u32, data: &[u8]) -> Result<(), ()> {
        use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
        let mut dfu = self.dfu_partition.clone();
        if !self.erased {
            let cap = dfu.capacity() as u32;
            dfu.erase(0, cap).map_err(|_| ())?;
            self.erased = true;
        }
        dfu.write(offset, data).map_err(|_| ())
    }

    /// Mark firmware as valid and reset into the new image.
    pub fn mark_updated_and_reset(&self) -> Result<(), ()> {
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

/// Global pointer to the peripheral firmware binary (set by central).
#[cfg(feature = "dfu_split")]
static FW_UPDATE_PTR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
/// Length of the peripheral firmware binary.
#[cfg(feature = "dfu_split")]
static FW_UPDATE_LEN: AtomicU32 = AtomicU32::new(0);
/// CRC32 of the peripheral firmware binary.
#[cfg(feature = "dfu_split")]
static FW_UPDATE_HASH: AtomicU32 = AtomicU32::new(0);

/// Store a reference to the peripheral firmware binary and its CRC32 hash.
///
/// The central calls this before starting the split peripheral manager so
/// that `PeripheralManager` can verify and update the peripheral's firmware
/// at connection time.
#[cfg(feature = "dfu_split")]
pub fn set_firmware_update_data(firmware: &'static [u8], hash: u32) {
    FW_UPDATE_PTR.store(firmware.as_ptr() as *mut u8, Ordering::Release);
    FW_UPDATE_LEN.store(firmware.len() as u32, Ordering::Release);
    FW_UPDATE_HASH.store(hash, Ordering::Release);
}

/// Retrieve the stored peripheral firmware data, if any.
#[cfg(feature = "dfu_split")]
pub fn get_firmware_update_data() -> Option<(&'static [u8], u32)> {
    let ptr = FW_UPDATE_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    let len = FW_UPDATE_LEN.load(Ordering::Acquire) as usize;
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    let hash = FW_UPDATE_HASH.load(Ordering::Acquire);
    Some((slice, hash))
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
