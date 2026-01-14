//! Power management events

use rmk_macro::controller_event;

/// Battery level changed event
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryLevelEvent {
    pub level: u8,
}

/// Charging state changed event
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChargingStateEvent {
    pub charging: bool,
}
