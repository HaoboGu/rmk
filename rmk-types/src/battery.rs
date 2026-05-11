//! Battery status types.

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// Charge state of the battery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
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
