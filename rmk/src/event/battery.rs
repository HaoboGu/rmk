//! Battery related events
//!
//! This module contains all battery-related events:
//! - Battery ADC reading events
//! - Charging state events
//! - Battery state events (computed from ADC and charging state)

use postcard::experimental::max_size::MaxSize;
use rmk_macro::event;
use rmk_types::event::BatteryStatus;
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

impl_payload_wrapper!(BatteryStatusEvent, BatteryStatus);
