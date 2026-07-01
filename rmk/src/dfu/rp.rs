use core::cell::RefCell;

use embassy_boot::BlockingFirmwareState;
use embassy_rp::{
    flash::{Blocking, Flash},
    gpio::Output,
    peripherals::FLASH,
};
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_sync::once_lock::OnceLock;
use static_cell::StaticCell;

use super::{DfuFlashManager, MutexType, PartitionType};

/// Total flash size passed to the embassy-rp Flash const generic.
pub const FLASH_SIZE: usize = 16 * 1024 * 1024;

/// Flash write granularity — 1 for RP2040.
pub const DFU_WRITE_SIZE: usize = 1;

static FLASH_CELL: StaticCell<MutexType> = StaticCell::new();
static MANAGER: OnceLock<DfuFlashManager> = OnceLock::new();
static LED: OnceLock<Mutex<CriticalSectionRawMutex, RefCell<Option<Output<'static>>>>> = OnceLock::new();

/// Initialize the blocking flash, create the DFU manager and store it globally.
pub fn init_flash(
    flash_peri: Peri<'static, FLASH>,
    storage_offset: u32,
    storage_size: u32,
    state_offset: u32,
    state_size: u32,
    dfu_offset: u32,
    dfu_size: u32,
) -> PartitionType {
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
pub fn get_manager() -> Option<&'static DfuFlashManager> {
    MANAGER.try_get()
}

/// Store an optional DFU LED pin globally.
pub fn set_led(led: Option<Output<'static>>) {
    let _ = LED.init(Mutex::new(RefCell::new(led)));
}

/// Run a closure with the global DFU LED, if configured.
pub fn with_led<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Output<'static>) -> R,
{
    LED.try_get()
        .and_then(|m| m.lock(|cell| cell.borrow_mut().as_mut().map(f)))
}
