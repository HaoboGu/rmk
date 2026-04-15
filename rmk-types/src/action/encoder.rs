//! Rotary encoder actions.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::KeyAction;

/// Action for a rotary encoder position, stored in the encoder map.
///
/// Both fields default to `KeyAction::No` (no action).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
pub struct EncoderAction {
    /// Action triggered when the encoder is rotated clockwise.
    pub clockwise: KeyAction,
    /// Action triggered when the encoder is rotated counter-clockwise.
    pub counter_clockwise: KeyAction,
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
}
