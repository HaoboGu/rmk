use core::cell::RefCell;

use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_nrf::Peri;
use embassy_nrf::nvmc::Nvmc;
use embassy_nrf::peripherals::NVMC;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

/// The internal flash peripheral token — `p.NVMC` — handed to
/// `rmk::dfu::init_flash_from_linkerscript` by the user.
pub type FlashType = Peri<'static, NVMC>;
/// The internal flash driver instance, built from [`FlashType`] by
/// [`build_driver`].
pub(super) type FlashDriver = Nvmc<'static>;
/// Mutex-wrapped internal flash, shared between DFU and storage partitions.
pub type MutexType = Mutex<CriticalSectionRawMutex, RefCell<FlashDriver>>;
/// `BlockingPartition` over the internal flash.
pub type PartitionType = BlockingPartition<'static, CriticalSectionRawMutex, FlashDriver>;

/// Build the internal flash driver from the raw peripheral token.
pub(super) fn build_driver(peri: FlashType) -> FlashDriver {
    Nvmc::new(peri)
}