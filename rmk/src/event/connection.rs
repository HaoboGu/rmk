//! Connection related events
//!
//! This module contains all connection-related events:
//! - Connection type change events (USB/BLE)
//! - BLE status change events

use core::ops::Deref;

use rmk_macro::event;
// Re-exported for convenience; canonical source is `rmk_types::connection::ConnectionType`.
pub use rmk_types::connection::ConnectionType;
#[cfg(feature = "_ble")]
use rmk_types::ble::{BleState, BleStatus};
use rmk_types::event::ConnectionPayload;
#[cfg(feature = "_ble")]
use rmk_types::event::{BleProfilePayload, BleStatePayload};

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

#[cfg(feature = "_ble")]
impl BleStatusChangeEvent {
    pub fn new(profile: u8, state: BleState) -> Self {
        Self(BleStatus { profile, state })
    }
}

#[cfg(feature = "_ble")]
impl Deref for BleStatusChangeEvent {
    type Target = BleStatus;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "_ble")]
impl From<BleStatusChangeEvent> for BleStatus {
    fn from(event: BleStatusChangeEvent) -> Self {
        event.0
    }
}

#[cfg(feature = "_ble")]
impl From<BleStatus> for BleStatusChangeEvent {
    fn from(status: BleStatus) -> Self {
        Self(status)
    }
}

#[cfg(feature = "_ble")]
impl From<BleStatusChangeEvent> for BleStatePayload {
    fn from(event: BleStatusChangeEvent) -> Self {
        Self {
            profile: event.profile,
            connected: matches!(event.state, BleState::Connected),
            advertising: matches!(event.state, BleState::Advertising),
        }
    }
}

/// BLE profile changed event
#[cfg(feature = "_ble")]
#[event(channel_size = crate::BLE_PROFILE_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::BLE_PROFILE_CHANGE_EVENT_PUB_SIZE, subs = crate::BLE_PROFILE_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleProfileChangeEvent(pub BleProfilePayload);

#[cfg(feature = "_ble")]
impl BleProfileChangeEvent {
    pub fn new(profile: u8) -> Self {
        Self(BleProfilePayload { profile })
    }
}

#[cfg(feature = "_ble")]
impl Deref for BleProfileChangeEvent {
    type Target = BleProfilePayload;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "_ble")]
impl From<BleProfileChangeEvent> for BleProfilePayload {
    fn from(event: BleProfileChangeEvent) -> Self {
        event.0
    }
}

#[cfg(feature = "_ble")]
impl From<BleProfilePayload> for BleProfileChangeEvent {
    fn from(payload: BleProfilePayload) -> Self {
        Self(payload)
    }
}
