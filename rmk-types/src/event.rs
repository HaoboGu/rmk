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

impl From<bool> for ChargeState {
    /// `true` = Charging, `false` = Discharging.
    fn from(charging: bool) -> Self {
        if charging {
            ChargeState::Charging
        } else {
            ChargeState::Discharging
        }
    }
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

impl BatteryStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, BatteryStatus::Available { .. })
    }
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

/// BLE state (what the BLE subsystem is currently doing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BleState {
    /// The BLE is advertising.
    Advertising,
    /// The BLE is connected.
    Connected,
    /// The BLE is not in use (USB mode or sleep mode, default).
    Inactive,
}

/// Unified BLE status: which profile is active and what the BLE is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleStatus {
    pub profile: u8,
    pub state: BleState,
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
