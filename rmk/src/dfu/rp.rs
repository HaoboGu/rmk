use core::cell::RefCell;

use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_rp::Peri;
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::peripherals::FLASH;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

/// Total flash size passed to the embassy-rp Flash const generic.
///
/// Set to 16 MB (the maximum common RP2040 flash size) so that the same
/// binary works on boards with 2, 4, 8 or 16 MB flash.  `new_blocking()`
/// ignores this value at runtime — it is only used for software bounds
/// checking inside embassy-rp.  Because all flash access goes through
/// `BlockingPartition` (which has its own partition-sized bounds checks),
/// overshooting the const generic is safe.
pub const FLASH_SIZE: usize = 16 * 1024 * 1024;

/// The internal flash peripheral token — `p.FLASH` — handed to
/// `rmk::dfu::init_flash_from_linkerscript` by the user.
pub type FlashType = Peri<'static, FLASH>;
/// The internal flash driver instance, built from [`FlashType`] by
/// [`build_driver`].
pub(super) type FlashDriver = Flash<'static, FLASH, Blocking, FLASH_SIZE>;
/// Mutex-wrapped internal flash, shared between DFU and storage partitions.
pub type MutexType = Mutex<CriticalSectionRawMutex, RefCell<FlashDriver>>;
/// `BlockingPartition` over the internal flash.
pub type PartitionType = BlockingPartition<'static, CriticalSectionRawMutex, FlashDriver>;

/// Build the internal flash driver from the raw peripheral token.
pub(super) fn build_driver(peri: FlashType) -> FlashDriver {
    Flash::<_, Blocking, FLASH_SIZE>::new_blocking(peri)
}
