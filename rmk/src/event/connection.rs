//! Connection related events
//!
//! This module contains all connection-related events:
//! - Connection type change events (USB/BLE)
//! - BLE status change events

use core::ops::Deref;

use rmk_macro::event;
pub use rmk_types::connection::ConnectionType;
use rmk_types::event::ConnectionPayload;

#[cfg(feature = "_ble")]
use rmk_types::ble::BleStatus;

// ============================================================================
// Connection Type Events
// ============================================================================

/// Connection type changed event
#[event(channel_size = crate::CONNECTION_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::CONNECTION_CHANGE_EVENT_PUB_SIZE, subs = crate::CONNECTION_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionChangeEvent(pub ConnectionPayload);

impl ConnectionChangeEvent {
    pub fn new(connection_type: ConnectionType) -> Self {
        Self(ConnectionPayload { connection_type })
    }
}

impl Deref for ConnectionChangeEvent {
    type Target = ConnectionPayload;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<ConnectionChangeEvent> for ConnectionPayload {
    fn from(event: ConnectionChangeEvent) -> Self {
        event.0
    }
}

impl From<ConnectionPayload> for ConnectionChangeEvent {
    fn from(payload: ConnectionPayload) -> Self {
        Self(payload)
    }
}

// ============================================================================
// BLE Connection Events
// ============================================================================

/// BLE status changed event
#[cfg(feature = "_ble")]
#[event(channel_size = crate::BLE_STATUS_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::BLE_STATUS_CHANGE_EVENT_PUB_SIZE, subs = crate::BLE_STATUS_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleStatusChangeEvent(pub BleStatus);
