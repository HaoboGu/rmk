//! Battery related events
//!
//! This module contains all battery-related events:
//! - Battery ADC reading events
//! - Charging state events
//! - Battery state events (computed from ADC and charging state)

use postcard::experimental::max_size::MaxSize;
use rmk_macro::event;
use rmk_types::event::{BatteryStatus, ChargeState};
use serde::{Deserialize, Serialize};

/// Battery adc read value
#[event(
    channel_size = crate::BATTERY_ADC_EVENT_CHANNEL_SIZE,
    pubs = crate::BATTERY_ADC_EVENT_PUB_SIZE,
    subs = crate::BATTERY_ADC_EVENT_SUB_SIZE
)]
#[derive(Serialize, Deserialize, Clone, Debug, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryAdcEvent(pub u16);

/// Charging state changed event
#[event(
    channel_size = crate::CHARGING_STATE_EVENT_CHANNEL_SIZE,
    pubs = crate::CHARGING_STATE_EVENT_PUB_SIZE,
    subs = crate::CHARGING_STATE_EVENT_SUB_SIZE
)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChargingStateEvent {
    pub charging: bool,
}

/// Battery state changed event, wraps [`BatteryStatus`].
#[event(channel_size = crate::BATTERY_STATE_EVENT_CHANNEL_SIZE, pubs = crate::BATTERY_STATE_EVENT_PUB_SIZE, subs = crate::BATTERY_STATE_EVENT_SUB_SIZE)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryStatusEvent(pub BatteryStatus);

impl BatteryStatusEvent {
    /// Battery state is not yet available.
    pub fn unavailable() -> Self {
        Self(BatteryStatus::Unavailable)
    }

    /// Returns `true` if the battery status is available.
    pub fn is_available(&self) -> bool {
        matches!(self.0, BatteryStatus::Available { .. })
    }

    /// Returns the battery level if available.
    pub fn level(&self) -> Option<u8> {
        match self.0 {
            BatteryStatus::Available { level, .. } => level,
            BatteryStatus::Unavailable => None,
        }
    }

    /// Returns the charge state if available.
    pub fn charge_state(&self) -> Option<ChargeState> {
        match self.0 {
            BatteryStatus::Available { charge_state, .. } => Some(charge_state),
            BatteryStatus::Unavailable => None,
        }
    }
}

impl From<BatteryStatusEvent> for BatteryStatus {
    fn from(event: BatteryStatusEvent) -> Self {
        event.0
    }
}

impl From<BatteryStatus> for BatteryStatusEvent {
    fn from(status: BatteryStatus) -> Self {
        Self(status)
    }
}
