use postcard::experimental::max_size::MaxSize;
use rmk_macro::event;
use serde::{Deserialize, Serialize};

/// Battery adc read value
#[event(channel_size = crate::BATTERY_ADC_EVENT_CHANNEL_SIZE)]
#[derive(Serialize, Deserialize, Clone, Debug, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryAdcEvent(pub u16);

/// Charging state changed event
#[event(channel_size = crate::CHARGING_STATE_EVENT_CHANNEL_SIZE)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChargingStateEvent {
    pub charging: bool,
}
