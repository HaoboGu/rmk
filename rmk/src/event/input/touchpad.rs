use postcard::experimental::max_size::MaxSize;
use rmk_macro::input_event;
use serde::{Deserialize, Serialize};

use crate::event::AxisEvent;

/// Event for multi-touch touchpad
#[input_event(channel_size = 8)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TouchpadEvent {
    /// Finger slot
    pub finger: u8,
    /// X, Y axes for touchpad
    pub axis: [AxisEvent; 2],
}
