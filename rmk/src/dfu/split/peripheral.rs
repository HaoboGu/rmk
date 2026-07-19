use core::sync::atomic::Ordering;

use rmk_types::dfu::DfuStatus;
use static_cell::StaticCell;

use super::super::{PartitionType, get_manager};
use crate::event::{DfuStatusEvent, publish_event};

// =========================================================================
// SplitDfuHandler — peripheral-side firmware writing
// =========================================================================

/// Handles firmware chunk writes on the **peripheral** side of a split
/// keyboard during an over-the-split-link DFU update.
///
/// Created on demand when the first [`SplitMessage::FirmwareChunk`]
/// arrives.  Flash pages are erased incrementally (only when a new page
/// boundary is hit), so the first chunk does not stall the split link
/// for the full erase time.
///
/// # Lifecycle
///
/// 1. [`SplitDfuHandler::new`] — acquire partition handles from
///    the global [`DfuFlashManager`].
/// 2. [`write_chunk`](SplitDfuHandler::write_chunk) — erase + write.
/// 3. [`compute_dfu_crc`](SplitDfuHandler::compute_dfu_crc) — read
///    back the entire DFU partition and return its CRC-32.
/// 4. [`mark_updated_and_reset`](SplitDfuHandler::mark_updated_and_reset)
///    — tell embassy-boot the new firmware is valid, then reset into it.
pub struct SplitDfuHandler {
    dfu_partition: PartitionType,
    state_partition: PartitionType,
    last_erased_page: Option<u32>,
    written_len: u32,
}

impl SplitDfuHandler {
    /// Create a new handler from the global [`DfuFlashManager`].
    /// Returns `None` if `init_flash` has not been called yet.
    pub fn new() -> Option<Self> {
        let mgr = get_manager()?;
        Some(Self {
            dfu_partition: mgr.dfu_partition(),
            state_partition: mgr.state_partition(),
            last_erased_page: None,
            written_len: 0,
        })
    }

    /// Write a chunk of firmware data at the given partition offset.
    ///
    /// Pages are erased on demand — only the first time a particular page
    /// is encountered.  This avoids a long blocking erase of the entire
    /// DFU partition on the very first chunk.
    pub fn write_chunk(&mut self, offset: u32, data: &[u8]) -> Result<(), ()> {
        use embedded_storage::nor_flash::NorFlash;
        let mut dfu = self.dfu_partition.clone();
        let erase_size = <PartitionType as NorFlash>::ERASE_SIZE as u32;
        let start_page = offset / erase_size;
        let end = offset + data.len() as u32;
        let end_page = (end - 1) / erase_size;
        for page in start_page..=end_page {
            if self.last_erased_page != Some(page) {
                dfu.erase(page * erase_size, (page + 1) * erase_size).map_err(|_| ())?;
                self.last_erased_page = Some(page);
            }
        }
        dfu.write(offset, data).map_err(|_| ())?;
        self.written_len = self.written_len.max(offset + data.len() as u32);
        publish_event(DfuStatusEvent::new(DfuStatus::Downloading));
        Ok(())
    }

    /// Read back the entire DFU partition and compute its CRC-32.
    ///
    /// Called by the peripheral during end-to-end verification before
    /// resetting into the new firmware.  Only the bytes up to the
    /// highest written offset are included.
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

    /// Mark the new firmware as valid and reset into it.
    ///
    /// Calls `embassy-boot`'s `mark_updated` and then performs a
    /// system reset.  The bootloader will copy the DFU slot to the
    /// active slot on the next boot.
    pub fn mark_updated_and_reset(&self) -> Result<(), ()> {
        #[cfg(feature = "dfu_nrf")]
        use embassy_boot::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
        #[cfg(feature = "dfu_rp")]
        use embassy_boot_rp::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
        let config = FirmwareUpdaterConfig {
            dfu: self.dfu_partition.clone(),
            state: self.state_partition.clone(),
        };
        static ALIGNED: StaticCell<[u8; super::super::DFU_WRITE_SIZE]> = StaticCell::new();
        let mut updater = BlockingFirmwareUpdater::new(config, ALIGNED.init([0; super::super::DFU_WRITE_SIZE]));
        updater.mark_updated().map_err(|_| ())?;
        publish_event(DfuStatusEvent::new(DfuStatus::Finished));
        cortex_m::peripheral::SCB::sys_reset()
    }
}

/// Return the CRC-32 of the currently running firmware binary.
///
/// The result is computed once and cached.  The firmware region is
/// determined by the `__vector_table` / `__veneer_limit` linker symbols,
/// covering the entire `.text` + `.rodata` + `.data` sections.
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
