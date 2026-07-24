use crate::dfu::ble_dfu::{BleDfuHandler, DfuPartition};
use crate::dfu::rp::FlashType;

/// RP2040-specific async DFU partition backed by the embassy-rp blocking
/// flash.  Used by the BLE DFU GATT handler to write received firmware chunks
/// to the DFU flash partition.
pub(crate) type AsyncDfuPartition = DfuPartition<FlashType>;

impl DfuPartition<FlashType> {
    pub(crate) fn make_dfu_handler(mgr: &super::DfuFlashManager) -> BleDfuHandler<Self> {
        BleDfuHandler::new(
            Self::new(mgr.flash_mutex(), mgr.dfu_offset(), mgr.dfu_size()),
            mgr.dfu_size(),
            mgr.dfu_offset(),
        )
    }
}
