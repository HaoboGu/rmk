//! Connection related events
//!
//! This module contains all connection-related events:
//! - Connection type change events (USB/BLE)
//! - BLE status change events

use rmk_macro::event;
#[cfg(feature = "_ble")]
use rmk_types::ble::BleStatus;

// ============================================================================
// Connection Type Events
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectionType {
    Usb,
    Ble,
}

impl From<u8> for ConnectionType {
    fn from(value: u8) -> Self {
        match value {
            0 => ConnectionType::Usb,
            1 => ConnectionType::Ble,
            _ => ConnectionType::Usb,
        }
    }
}

impl From<ConnectionType> for u8 {
    fn from(value: ConnectionType) -> Self {
        match value {
            ConnectionType::Usb => 0,
            ConnectionType::Ble => 1,
        }
    }
}

/// Connection type changed event
#[event(channel_size = crate::CONNECTION_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::CONNECTION_CHANGE_EVENT_PUB_SIZE, subs = crate::CONNECTION_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionChangeEvent {
    pub connection_type: ConnectionType,
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
