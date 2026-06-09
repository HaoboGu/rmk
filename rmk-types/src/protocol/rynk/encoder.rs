//! Encoder endpoint types.

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::action::EncoderAction;

/// Request payload for `GetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct GetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
}

/// Request payload for `SetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct SetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
    pub action: EncoderAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, KeyAction};
    use crate::keycode::{ConsumerKey, KeyCode};
    use crate::protocol::rynk::test_utils::round_trip;

    #[test]
    fn round_trip_encoder_requests() {
        round_trip(&GetEncoderRequest {
            encoder_id: 0,
            layer: 1,
        });
        round_trip(&SetEncoderRequest {
            encoder_id: 0,
            layer: 1,
            action: EncoderAction::default(),
        });
        round_trip(&SetEncoderRequest {
            encoder_id: 1,
            layer: 2,
            action: EncoderAction::new(
                KeyAction::Single(Action::Key(KeyCode::Consumer(ConsumerKey::VolumeIncrement))),
                KeyAction::Single(Action::Key(KeyCode::Consumer(ConsumerKey::VolumeDecrement))),
            ),
        });
    }
}
