#[cfg(feature = "dfu_ble")]
pub(crate) fn make_dfu_handler(
    mgr: &crate::dfu::DfuFlashManager,
) -> crate::dfu::ble_dfu::BleDfuHandler<crate::dfu::ble_rp::AsyncDfuPartition> {
    crate::dfu::ble_rp::AsyncDfuPartition::make_dfu_handler(mgr)
}
