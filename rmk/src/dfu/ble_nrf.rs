use embassy_nrf::nvmc::Nvmc;

use crate::dfu::ble_dfu::{BleDfuHandler, DfuPartition};

/// nRF-specific async DFU partition backed by the internal NVMC flash
/// controller.  Used by the BLE DFU GATT handler to write received firmware
/// chunks to the DFU flash partition.
pub(crate) type AsyncDfuPartition = DfuPartition<Nvmc<'static>>;

impl DfuPartition<Nvmc<'static>> {
    pub(crate) fn make_dfu_handler(mgr: &super::DfuFlashManager) -> BleDfuHandler<Self> {
        BleDfuHandler::new(
            Self::new(mgr.flash_mutex(), mgr.dfu_offset(), mgr.dfu_size()),
            mgr.dfu_size(),
            mgr.dfu_offset(),
        )
    }
}
