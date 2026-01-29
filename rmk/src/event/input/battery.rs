use postcard::experimental::max_size::MaxSize;
use rmk_macro::input_event;
use serde::{Deserialize, Serialize};

/// Battery adc read value
///
#[input_event(channel_size = 2)]
#[derive(Serialize, Deserialize, Clone, Debug, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryEvent(pub u16);

/// Charging state changed event, true means charging, false means not charging
///
#[input_event(channel_size = 2)]
#[derive(Serialize, Deserialize, Clone, Debug, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChargingStateEvent {
    pub state: bool,
}
