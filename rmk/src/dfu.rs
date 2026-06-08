use core::cell::RefCell;
use core::sync::atomic::{AtomicPtr, Ordering};

use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::peripherals::FLASH;
use embassy_rp::Peri;
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

type FlashType = Flash<'static, FLASH, Blocking, FLASH_SIZE>;
type MutexType = Mutex<NoopRawMutex, RefCell<FlashType>>;
type PartitionType = BlockingPartition<'static, NoopRawMutex, FlashType>;

static FLASH_CELL: StaticCell<MutexType> = StaticCell::new();
static MANAGER_CELL: StaticCell<DfuFlashManager> = StaticCell::new();
static MANAGER_PTR: AtomicPtr<DfuFlashManager> = AtomicPtr::new(core::ptr::null_mut());

// Partition offsets relative to flash base (0x10000000), matching bootymcbootface/memory.x
const STATE_OFFSET: u32 = 0x6000;
const STATE_SIZE: u32 = 0x1000;
const DFU_OFFSET: u32 = 0x87000;
const DFU_SIZE: u32 = 516 * 1024;

pub struct DfuFlashManager {
    flash_mutex: &'static MutexType,
    state_offset: u32,
    state_size: u32,
    dfu_offset: u32,
    dfu_size: u32,
    storage_offset: u32,
    storage_size: u32,
}

impl DfuFlashManager {
    fn new(flash_mutex: &'static MutexType, storage_start: u32, storage_end: u32) -> Self {
        Self {
            flash_mutex,
            state_offset: STATE_OFFSET,
            state_size: STATE_SIZE,
            dfu_offset: DFU_OFFSET,
            dfu_size: DFU_SIZE,
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

/// Initialize the blocking flash, create the DFU manager and store it globally.
///
/// Returns the storage partition (blocking flash) which the caller should wrap
/// in `async_flash_wrapper` for use with `initialize_keymap_and_storage`.
pub fn init_flash(flash_peri: Peri<'static, FLASH>, storage_start: u32, storage_end: u32) -> PartitionType {
    let raw_flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(flash_peri);
    let flash_mutex: &'static MutexType = FLASH_CELL.init(Mutex::new(RefCell::new(raw_flash)));
    let mgr = MANAGER_CELL.init(DfuFlashManager::new(flash_mutex, storage_start, storage_end));
    MANAGER_PTR.store(mgr as *const _ as *mut _, Ordering::Release);
    mgr.storage_partition()
}

/// Mark firmware boot as successful so the bootloader doesn't revert on next reset.
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
pub fn get_manager() -> Option<&'static DfuFlashManager> {
    let ptr = MANAGER_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

/// Register a DFU interface on the USB builder using the given manager.
pub fn register_dfu_interface<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
    mgr: &'static DfuFlashManager,
    product_name: &'static str,
) {
    use embassy_boot_rp::BlockingFirmwareUpdater;
    use embassy_boot_rp::FirmwareUpdaterConfig;
    use embassy_usb::class::dfu::consts::DfuAttributes;
    use embassy_usb_dfu::{dfu, ResetImmediate};

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
    let state = dfu::new_state::<PartitionType, PartitionType, ResetImmediate, BLOCK_SIZE>(
        updater,
        attrs,
        ResetImmediate,
    );

    static DFU_STATE: StaticCell<
        dfu::State<'static, PartitionType, PartitionType, ResetImmediate, BLOCK_SIZE>,
    > = StaticCell::new();
    let state_ref = DFU_STATE.init(state);

    // Allocate a string index for the DFU interface name
    let string_idx = builder.string();

    // Manually set up the DFU interface with the string index.
    // We replicate dfu_mode::usb_dfu here to pass a non-None string.
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

    // Register a handler to serve the interface name string
    static STRING_PROVIDER: StaticCell<DfuStringProvider> = StaticCell::new();
    let string_provider = STRING_PROVIDER.init(DfuStringProvider {
        string_idx,
        string_val: product_name,
    });
    builder.handler(string_provider);
}
