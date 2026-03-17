//! BLE status types.

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// BLE state (what the BLE subsystem is currently doing).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleStatus {
    pub profile: u8,
    pub state: BleState,
}

impl Default for BleStatus {
    fn default() -> Self {
        Self {
            profile: 0,
            state: BleState::Inactive,
        }
    }
}
