//! Power management events

use rmk_macro::controller_event;

/// Battery level changed event
#[controller_event(channel_size = crate::BATTERY_LEVEL_EVENT_CHANNEL_SIZE, pubs = crate::BATTERY_LEVEL_EVENT_PUB_SIZE, subs = crate::BATTERY_LEVEL_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryLevelEvent {
    pub level: u8,
}

/// Charging state changed event
#[controller_event(channel_size = crate::CHARGING_STATE_EVENT_CHANNEL_SIZE, pubs = crate::CHARGING_STATE_EVENT_PUB_SIZE, subs = crate::CHARGING_STATE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChargingStateEvent {
    pub charging: bool,
}
