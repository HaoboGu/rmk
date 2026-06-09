use core::cell::RefCell;
use core::sync::atomic::{AtomicPtr, Ordering};
#[cfg(feature = "dfu_lock")]
use core::sync::atomic::AtomicBool;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_usb::types::StringIndex;
use embassy_usb::control::{InResponse, OutResponse, Request};
use embassy_usb::driver::Driver;
use embassy_usb::{Builder, Handler};
use static_cell::StaticCell;

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

/// Total flash size for RP2040.
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[cfg(feature = "dfu-rp")]
use embassy_embedded_hal::flash::partition::BlockingPartition;
#[cfg(feature = "dfu-rp")]
use embassy_rp::flash::{Blocking, Flash};
#[cfg(feature = "dfu-rp")]
use embassy_rp::gpio::Output;
#[cfg(feature = "dfu-rp")]
use embassy_rp::peripherals::FLASH;
#[cfg(feature = "dfu-rp")]
use embassy_rp::Peri;
#[cfg(feature = "dfu-rp")]
use embassy_usb::class::dfu::consts::Status;
#[cfg(feature = "dfu-rp")]
use embassy_usb::class::dfu::dfu_mode::{self, DfuState};

#[cfg(feature = "dfu-rp")]
type FlashType = Flash<'static, FLASH, Blocking, FLASH_SIZE>;
#[cfg(feature = "dfu-rp")]
type MutexType = Mutex<NoopRawMutex, RefCell<FlashType>>;
#[cfg(feature = "dfu-rp")]
type PartitionType = BlockingPartition<'static, NoopRawMutex, FlashType>;

#[cfg(feature = "dfu-rp")]
static FLASH_CELL: StaticCell<MutexType> = StaticCell::new();
#[cfg(feature = "dfu-rp")]
static MANAGER_CELL: StaticCell<DfuFlashManager> = StaticCell::new();
#[cfg(feature = "dfu-rp")]
static MANAGER_PTR: AtomicPtr<DfuFlashManager> = AtomicPtr::new(core::ptr::null_mut());
#[cfg(feature = "dfu-rp")]
static LED_MUTEX: AtomicPtr<Mutex<NoopRawMutex, RefCell<Option<Output<'static>>>>> =
    AtomicPtr::new(core::ptr::null_mut());

#[cfg(feature = "dfu-rp")]
pub struct DfuFlashManager {
    flash_mutex: &'static MutexType,
    state_offset: u32,
    state_size: u32,
    dfu_offset: u32,
    dfu_size: u32,
    storage_offset: u32,
    storage_size: u32,
}

#[cfg(feature = "dfu-rp")]
impl DfuFlashManager {
    fn new(
        flash_mutex: &'static MutexType,
        storage_start: u32,
        storage_end: u32,
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
            storage_offset: storage_start,
            storage_size: storage_end - storage_start,
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

/// Store an optional DFU LED pin globally.
#[cfg(feature = "dfu-rp")]
pub fn set_led(led: Option<Output<'static>>) {
    static LED_CELL: StaticCell<Mutex<NoopRawMutex, RefCell<Option<Output<'static>>>>> =
        StaticCell::new();
    let m = LED_CELL.init(Mutex::new(RefCell::new(led)));
    LED_MUTEX.store(m as *const _ as *mut _, Ordering::Release);
}

/// Run a closure with the global DFU LED, if configured.
#[cfg(feature = "dfu-rp")]
fn with_led<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Output<'static>) -> R,
{
    let ptr = LED_MUTEX.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    let m = unsafe { &*ptr };
    m.lock(|cell| cell.borrow_mut().as_mut().map(f))
}

/// DFU lock state (shared between keyboard loop and DFU handler).
#[cfg(feature = "dfu_lock")]
static DFU_LOCKED: AtomicBool = AtomicBool::new(true);

/// Set to true once `RmkDfuHandler::start()` begins a DFU download.
#[cfg(feature = "dfu_lock")]
static DFU_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "dfu_lock")]
pub fn is_dfu_unlocked() -> bool {
    !DFU_LOCKED.load(Ordering::Acquire)
}

/// DFU handler wrapper that blinks an LED during transfer and checks the
/// DFU lock (if `dfu_lock` feature is enabled).
#[cfg(feature = "dfu-rp")]
struct RmkDfuHandler<H> {
    inner: H,
}

#[cfg(feature = "dfu-rp")]
impl<H: dfu_mode::Handler> dfu_mode::Handler for RmkDfuHandler<H> {
    fn start(&mut self) -> Result<(), Status> {
        #[cfg(feature = "dfu_lock")]
        if !is_dfu_unlocked() {
            info!("dfu_lock: DFU download rejected — keys not unlocked");
            return Err(Status::ErrVendor);
        }
        #[cfg(feature = "dfu_lock")]
        DFU_STARTED.store(true, Ordering::Release);
        info!("dfu_lock: DFU download started");
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

/// Initialize the blocking flash, create the DFU manager and store it globally.
#[cfg(feature = "dfu-rp")]
pub fn init_flash(
    flash_peri: Peri<'static, FLASH>,
    storage_start: u32,
    storage_end: u32,
    state_offset: u32,
    state_size: u32,
    dfu_offset: u32,
    dfu_size: u32,
) -> PartitionType {
    let raw_flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(flash_peri);
    let flash_mutex: &'static MutexType = FLASH_CELL.init(Mutex::new(RefCell::new(raw_flash)));
    let mgr = MANAGER_CELL.init(DfuFlashManager::new(
        flash_mutex,
        storage_start,
        storage_end,
        state_offset,
        state_size,
        dfu_offset,
        dfu_size,
    ));
    MANAGER_PTR.store(mgr as *const _ as *mut _, Ordering::Release);
    mgr.storage_partition()
}

/// Mark firmware boot as successful so the bootloader doesn't revert on next reset.
#[cfg(feature = "dfu-rp")]
pub fn mark_booted() {
    if let Some(mgr) = get_manager() {
        let state_part = mgr.state_partition();
        static ALIGNED: StaticCell<[u8; 1]> = StaticCell::new();
        let aligned: &'static mut [u8] = ALIGNED.init([0; 1]);
        let mut state = embassy_boot_rp::BlockingFirmwareState::new(state_part, aligned);
        state.mark_booted().ok();
    }
}

/// Retrieve the global DFU manager.
#[cfg(feature = "dfu-rp")]
pub fn get_manager() -> Option<&'static DfuFlashManager> {
    let ptr = MANAGER_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

/// Register a DFU interface on the USB builder.
#[cfg(feature = "dfu-rp")]
pub fn register_dfu_interface<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    mgr: &'static DfuFlashManager,
    product_name: &'static str,
) {
    use embassy_boot_rp::BlockingFirmwareUpdater;
    use embassy_boot_rp::FirmwareUpdaterConfig;
    use embassy_usb::class::dfu::consts::DfuAttributes;
    use embassy_usb_dfu::ResetImmediate;
    use embassy_usb_dfu::dfu::FirmwareHandler;

    let dfu_part = mgr.dfu_partition();
    let state_part = mgr.state_partition();
    let config = FirmwareUpdaterConfig {
        dfu: dfu_part,
        state: state_part,
    };
    static ALIGNED: StaticCell<[u8; 1]> = StaticCell::new();
    let aligned: &'static mut [u8] = ALIGNED.init([0; 1]);
    let updater = BlockingFirmwareUpdater::new(config, aligned);

    const BLOCK_SIZE: usize = 256;
    let attrs = DfuAttributes::CAN_DOWNLOAD | DfuAttributes::WILL_DETACH;

    let inner = FirmwareHandler::new(updater, ResetImmediate);
    let handler = RmkDfuHandler { inner };
    let state = DfuState::new(handler, attrs);

    type DfuStateInner = RmkDfuHandler<
        FirmwareHandler<'static, PartitionType, PartitionType, ResetImmediate, BLOCK_SIZE>,
    >;
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
            (BLOCK_SIZE & 0xff) as u8,
            ((BLOCK_SIZE >> 8) & 0xff) as u8,
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
// dfu_lock — physical key unlock for DFU firmware download
// ---------------------------------------------------------------------------

/// DfuLock state machine that checks a physical key combination to unlock DFU.
#[cfg(feature = "dfu_lock")]
pub struct DfuLock<'a> {
    unlocked: AtomicBool,
    unlock_keys: &'a [(u8, u8)],
}

#[cfg(feature = "dfu_lock")]
impl<'a> DfuLock<'a> {
    pub fn new(unlock_keys: &'a [(u8, u8)]) -> Self {
        Self {
            unlocked: AtomicBool::new(false),
            unlock_keys,
        }
    }

    /// Poll the unlock keys. On first detection of all keys pressed:
    /// unlock DFU, then loop Morse "D F U" for 10 s or until DFU download
    /// actually starts. If nothing starts within 10 s the lock re-engages.
    pub async fn process_unlock(&self, keymap: &'a crate::keymap::KeyMap<'a>) {
        if self.unlock_keys.is_empty() {
            return;
        }
        let all_pressed = self
            .unlock_keys
            .iter()
            .all(|(row, col)| keymap.read_matrix_key(*row, *col));
        if all_pressed && !self.unlocked.load(Ordering::Relaxed) {
            self.unlocked.store(true, Ordering::Release);
            DFU_LOCKED.store(false, Ordering::Release);
            info!("dfu_lock: unlock keys pressed, DFU unlocked for 10 s");
            use embassy_time::Timer;
            with_led(|led| led.set_high());
            Timer::after_millis(500).await;

            let deadline = embassy_time::Instant::now()
                + embassy_time::Duration::from_secs(10);
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
                morse_blink_dfu().await;
            }
        }
    }
}

/// Blink "D F U" in Morse code on the DFU LED.
#[cfg(feature = "dfu_lock")]
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
