//! Shared event payload types used across RMK crates.
//!
//! These types represent the data carried by firmware events and are also
//! serialized over the RMK protocol. They live here (rather than in
//! `protocol::rmk`) because they are domain types used throughout the core
//! firmware, not protocol-specific artifacts.

use serde::{Deserialize, Serialize};

use crate::connection::ConnectionType;
use crate::led_indicator::LedIndicator;

/// Charge state of the battery.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    postcard_schema::Schema,
    postcard::experimental::max_size::MaxSize,
)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChargeState {
    Charging,
    Discharging,
    Unknown,
}

/// Battery status used for both status queries and event notifications.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    postcard_schema::Schema,
    postcard::experimental::max_size::MaxSize,
)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BatteryStatus {
    Unavailable,
    Available {
        charge_state: ChargeState,
        level: Option<u8>,
    },
}

/// Payload for the layer change event/topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LayerChangePayload {
    pub layer: u8,
}

/// Payload for the WPM update event/topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WpmPayload {
    pub wpm: u16,
}

/// Payload for BLE state change event/topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleStatePayload {
    pub profile: u8,
    pub connected: bool,
    pub advertising: bool,
}

/// Payload for BLE profile change event/topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleProfilePayload {
    pub profile: u8,
}

/// Payload for connection change event/topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionPayload {
    pub connection_type: ConnectionType,
}

/// Payload for sleep state event/topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SleepPayload {
    pub sleeping: bool,
}

/// Payload for LED indicator event/topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LedPayload {
    pub indicator: LedIndicator,
}
