//! Rotary encoder actions.

use super::KeyAction;

/// EncoderAction is the action at a encoder position, stored in encoder_map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
pub struct EncoderAction {
    clockwise: KeyAction,
    counter_clockwise: KeyAction,
}

impl Default for EncoderAction {
    fn default() -> Self {
        Self {
            clockwise: KeyAction::No,
            counter_clockwise: KeyAction::No,
        }
    }
}

impl EncoderAction {
    /// Create a new encoder action.
    pub const fn new(clockwise: KeyAction, counter_clockwise: KeyAction) -> Self {
        Self {
            clockwise,
            counter_clockwise,
        }
    }

    /// Set the clockwise action.
    pub fn set_clockwise(&mut self, clockwise: KeyAction) {
        self.clockwise = clockwise;
    }

    /// Set the counter clockwise action.
    pub fn set_counter_clockwise(&mut self, counter_clockwise: KeyAction) {
        self.counter_clockwise = counter_clockwise;
    }

    /// Get the clockwise action.
    pub fn clockwise(&self) -> KeyAction {
        self.clockwise
    }

    /// Get the counter clockwise action.
    pub fn counter_clockwise(&self) -> KeyAction {
        self.counter_clockwise
    }
}
