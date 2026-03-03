//! Connection related events
//!
//! This module contains all connection-related events:
//! - Connection type change events (USB/BLE)
//! - BLE state change events
//! - BLE profile change events

use rmk_macro::event;
// Re-exported for convenience; canonical source is `rmk_types::connection::ConnectionType`.
pub use rmk_types::connection::ConnectionType;
use rmk_types::event::ConnectionPayload;

#[cfg(feature = "_ble")]
use crate::ble::BleState;
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

impl_payload_wrapper!(ConnectionChangeEvent, ConnectionPayload);

// ============================================================================
// BLE Connection Events
// ============================================================================

/// BLE state changed event
#[cfg(feature = "_ble")]
#[event(channel_size = crate::BLE_STATE_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::BLE_STATE_CHANGE_EVENT_PUB_SIZE, subs = crate::BLE_STATE_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleStateChangeEvent {
    pub profile: u8,
    pub state: BleState,
}

#[cfg(feature = "_ble")]
impl BleStateChangeEvent {
    pub fn new(profile: u8, state: BleState) -> Self {
        Self { profile, state }
    }
}

#[cfg(feature = "_ble")]
impl From<BleStateChangeEvent> for BleStatePayload {
    fn from(event: BleStateChangeEvent) -> Self {
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
impl_payload_wrapper!(BleProfileChangeEvent, BleProfilePayload);
