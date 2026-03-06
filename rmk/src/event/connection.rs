//! Connection related events
//!
//! This module contains all connection-related events:
//! - Connection type change events (USB/BLE)
//! - BLE status change events

use rmk_macro::event;
pub use rmk_types::connection::ConnectionType;

#[cfg(feature = "_ble")]
use rmk_types::ble::BleStatus;

// ============================================================================
// Connection Type Events
// ============================================================================

/// Connection type changed event
#[event(channel_size = crate::CONNECTION_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::CONNECTION_CHANGE_EVENT_PUB_SIZE, subs = crate::CONNECTION_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionChangeEvent(pub ConnectionType);

impl ConnectionChangeEvent {
    pub fn new(connection_type: ConnectionType) -> Self {
        Self(connection_type)
    }
}

impl_payload_wrapper!(ConnectionChangeEvent, ConnectionType);

// ============================================================================
// BLE Connection Events
// ============================================================================

/// BLE status changed event
#[cfg(feature = "_ble")]
#[event(channel_size = crate::BLE_STATUS_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::BLE_STATUS_CHANGE_EVENT_PUB_SIZE, subs = crate::BLE_STATUS_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleStatusChangeEvent(pub BleStatus);

#[cfg(feature = "_ble")]
impl_payload_wrapper!(BleStatusChangeEvent, BleStatus);
