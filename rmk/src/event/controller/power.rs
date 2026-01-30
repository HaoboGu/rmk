//! Power management events

use postcard::experimental::max_size::MaxSize;
use rmk_macro::{controller_event, input_event};
use serde::{Deserialize, Serialize};

/// Battery level changed event
#[controller_event(channel_size = crate::BATTERY_LEVEL_EVENT_CHANNEL_SIZE, pubs = crate::BATTERY_LEVEL_EVENT_PUB_SIZE, subs = crate::BATTERY_LEVEL_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryLevelEvent {
    pub level: u8,
}

/// Charging state changed event
///
/// This event supports both input and controller patterns:
/// - As InputEvent: used by split peripherals to send charging state to central
/// - As ControllerEvent: used to broadcast charging state changes to controllers
#[input_event(channel_size = 2)]
#[controller_event(channel_size = crate::CHARGING_STATE_EVENT_CHANNEL_SIZE, pubs = crate::CHARGING_STATE_EVENT_PUB_SIZE, subs = crate::CHARGING_STATE_EVENT_SUB_SIZE)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChargingStateEvent {
    pub charging: bool,
}
