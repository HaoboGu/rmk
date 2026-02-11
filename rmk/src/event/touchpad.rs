use postcard::experimental::max_size::MaxSize;
use rmk_macro::event;
use serde::{Deserialize, Serialize};

use crate::event::AxisEvent;

/// Event for multi-touch touchpad
#[event(
    channel_size = crate::TOUCHPAD_EVENT_CHANNEL_SIZE,
    pubs = crate::TOUCHPAD_EVENT_PUB_SIZE,
    subs = crate::TOUCHPAD_EVENT_SUB_SIZE
)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TouchpadEvent {
    /// Finger slot
    pub finger: u8,
    /// X, Y axes for touchpad
    pub axis: [AxisEvent; 2],
}
