//! Power management events

use postcard::experimental::max_size::MaxSize;
use rmk_macro::controller_event;
use serde::{Deserialize, Serialize};

/// Battery state changed event
#[controller_event(channel_size = crate::BATTERY_STATE_EVENT_CHANNEL_SIZE, pubs = crate::BATTERY_STATE_EVENT_PUB_SIZE, subs = crate::BATTERY_STATE_EVENT_SUB_SIZE)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BatteryStateEvent {
    /// The battery state is not available yet
    NotAvailable,
    /// Normal battery level, value range is 0~100
    Normal(u8),
    /// Battery is currently charging
    Charging,
    /// Charging completed, battery level is 100
    Charged,
}
