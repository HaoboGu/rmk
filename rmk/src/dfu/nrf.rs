use core::cell::RefCell;

use embassy_boot::BlockingFirmwareState;
#[cfg(feature = "dfu_ble")]
use embassy_boot::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_nrf::Peri;
use embassy_nrf::nvmc::Nvmc;
use embassy_nrf::peripherals::NVMC;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::once_lock::OnceLock;
use static_cell::StaticCell;

use super::DfuFlashManager;

/// Flash write granularity — 4 for nRF NVMC.
pub const DFU_WRITE_SIZE: usize = 4;

pub(super) type FlashType = Nvmc<'static>;
pub(super) type MutexType = Mutex<CriticalSectionRawMutex, RefCell<FlashType>>;
pub(super) type PartitionType = BlockingPartition<'static, CriticalSectionRawMutex, FlashType>;

static FLASH_CELL: StaticCell<MutexType> = StaticCell::new();
static MANAGER: OnceLock<DfuFlashManager> = OnceLock::new();

/// Initialize the blocking flash, create the DFU manager and store it globally.
pub fn init_flash(
    flash_peri: Peri<'static, NVMC>,
    storage_offset: u32,
    storage_size: u32,
    state_offset: u32,
    state_size: u32,
    dfu_offset: u32,
    dfu_size: u32,
) -> PartitionType {
    let raw_flash = Nvmc::new(flash_peri);

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

/// Mark the DFU firmware as valid and reset the MCU.
#[cfg(feature = "dfu_ble")]
pub fn mark_updated_and_reset() {
    info!("dfu: marking firmware as updated...");
    if let Some(mgr) = get_manager() {
        let dfu_part = mgr.dfu_partition();
        let state_part = mgr.state_partition();
        static ALIGNED: StaticCell<[u8; DFU_WRITE_SIZE]> = StaticCell::new();
        let aligned: &'static mut [u8] = ALIGNED.init([0; DFU_WRITE_SIZE]);
        let config = FirmwareUpdaterConfig {
            dfu: dfu_part,
            state: state_part,
        };
        let mut updater = BlockingFirmwareUpdater::new(config, aligned);
        match updater.mark_updated() {
            Ok(()) => info!("dfu: mark_updated succeeded, resetting now"),
            Err(e) => {
                #[cfg(feature = "defmt")]
                let e = defmt::Debug2Format(&e);
                error!("dfu: mark_updated failed: {:?}", e);
            }
        }
    } else {
        error!("dfu: no flash manager, cannot mark updated");
    }
    cortex_m::peripheral::SCB::sys_reset();
}
