//! GATT service definitions for the active host protocol.
//!
//! Each protocol's GATT characteristic layout and BLE write dispatch live in
//! its submodule (`vial`, `rynk`). This module re-exports the active
//! protocol's type as `HostGattService` and provides two cfg-gated forwarders
//! (`handle_write`, `host_cccd_handle`) so the BLE event loop doesn't need
//! to know which protocol is compiled in.

#[cfg(feature = "rmk_protocol")]
pub(crate) mod rynk;
#[cfg(feature = "vial")]
pub(crate) mod vial;

#[cfg(feature = "vial")]
pub(crate) use vial::VialGattService as HostGattService;

#[cfg(feature = "rmk_protocol")]
pub(crate) use rynk::RynkGattService as HostGattService;

/// Handle a GATT write event targeted at the active host protocol.
///
/// Returns `true` when the event was consumed.
#[cfg(feature = "vial")]
pub(crate) async fn handle_write(
    server: &crate::ble::ble_server::Server<'_>,
    event_handle: u16,
    event_data: &[u8],
) -> bool {
    vial::handle_write(&server.host_gatt, event_handle, event_data).await
}

#[cfg(feature = "rmk_protocol")]
pub(crate) async fn handle_write(
    server: &crate::ble::ble_server::Server<'_>,
    event_handle: u16,
    event_data: &[u8],
) -> bool {
    rynk::handle_write(&server.host_gatt, event_handle, event_data).await
}

/// GATT attribute handle of the active host protocol's notifiable
/// characteristic's CCCD. Used by the BLE event loop to recognise CCCD writes.
#[cfg(feature = "vial")]
pub(crate) fn host_cccd_handle(server: &crate::ble::ble_server::Server<'_>) -> u16 {
    vial::host_cccd_handle(&server.host_gatt)
}

#[cfg(feature = "rmk_protocol")]
pub(crate) fn host_cccd_handle(server: &crate::ble::ble_server::Server<'_>) -> u16 {
    rynk::host_cccd_handle(&server.host_gatt)
}
