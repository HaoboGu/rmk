//! GATT service definitions for the active host protocol.
//!
//! The active protocol's GATT characteristic layout and BLE write dispatch live
//! in its submodule (`vial`) and implements [`HostGatt`]. This module re-exports
//! the active protocol's type as `HostGattService` so the BLE event loop can
//! invoke trait methods without knowing which protocol is compiled in.

#[cfg(feature = "vial")]
pub(crate) mod vial;

#[cfg(feature = "vial")]
pub(crate) use vial::VialGattService as HostGattService;

/// Behavior shared by every host protocol's GATT service.
///
/// Implementors expose the CCCD handle of their notifiable characteristic and
/// consume GATT writes targeted at their own characteristics. The BLE event
/// loop calls these methods on `server.host_gatt` without caring which
/// concrete protocol is active.
pub(crate) trait HostGatt {
    /// GATT attribute handle of this protocol's notifiable characteristic's
    /// CCCD. Used by the BLE event loop to recognise CCCD writes.
    fn host_cccd_handle(&self) -> u16;

    /// Handle a GATT write targeted at this protocol's service.
    ///
    /// Returns `true` when the event was consumed.
    async fn handle_write(&self, event_handle: u16, event_data: &[u8]) -> bool;
}
