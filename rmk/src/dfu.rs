use core::cell::RefCell;
use core::sync::atomic::{AtomicPtr, Ordering};

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
static LED_MUTEX: AtomicPtr<Mutex<NoopRawMutex, RefCell<Option<Output<'static>>>>> = AtomicPtr::new(core::ptr::null_mut());

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

/// DFU handler wrapper that toggles an optional LED on start/finish/reset.
#[cfg(feature = "dfu-rp")]
struct RmkDfuHandler<H> {
    inner: H,
    led: Option<Output<'static>>,
}

#[cfg(feature = "dfu-rp")]
impl<H: dfu_mode::Handler> dfu_mode::Handler for RmkDfuHandler<H> {
    fn start(&mut self) -> Result<(), Status> {
        if let Some(ref mut led) = self.led {
            led.set_high();
        }
        self.inner.start()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Status> {
        self.inner.write(data)
    }

    fn finish(&mut self) -> Result<(), Status> {
        let res = self.inner.finish();
        if let Some(ref mut led) = self.led {
            led.set_low();
        }
        res
    }

    fn system_reset(&mut self) {
        if let Some(ref mut led) = self.led {
            led.set_low();
        }
        self.inner.system_reset()
    }
}

/// Store an optional DFU LED pin for use by the DFU USB handler.
///
/// Must be called before `take_led()` (i.e. before USB setup).
#[cfg(feature = "dfu-rp")]
pub fn set_led(led: Option<Output<'static>>) {
    static LED_CELL: StaticCell<Mutex<NoopRawMutex, RefCell<Option<Output<'static>>>>> = StaticCell::new();
    let m = LED_CELL.init(Mutex::new(RefCell::new(led)));
    LED_MUTEX.store(m as *const _ as *mut _, Ordering::Release);
}

/// Take the DFU LED (consumed once during USB setup).
#[cfg(feature = "dfu-rp")]
pub(crate) fn take_led() -> Option<Output<'static>> {
    let ptr = LED_MUTEX.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    let m = unsafe { &*ptr };
    m.lock(|cell| cell.borrow_mut().take())
}

/// Initialize the blocking flash, create the DFU manager and store it globally.
///
/// Returns the storage partition (blocking flash) which the caller should wrap
/// in `async_flash_wrapper` for use with `initialize_keymap_and_storage`.
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

/// Retrieve the global DFU manager (returns `None` before `init_flash` is called).
#[cfg(feature = "dfu-rp")]
pub fn get_manager() -> Option<&'static DfuFlashManager> {
    let ptr = MANAGER_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

/// Register a DFU interface on the USB builder using the given manager.
///
/// `led` is an optional GPIO output pin that is set high while a DFU
/// download is in progress and low when idle / finished.
#[cfg(feature = "dfu-rp")]
pub fn register_dfu_interface<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    mgr: &'static DfuFlashManager,
    product_name: &'static str,
    led: Option<Output<'static>>,
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
    let handler = RmkDfuHandler { inner, led };
    let state = DfuState::new(handler, attrs);

    type DfuStateInner = RmkDfuHandler<FirmwareHandler<
        'static,
        PartitionType,
        PartitionType,
        ResetImmediate,
        BLOCK_SIZE,
    >>;
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
