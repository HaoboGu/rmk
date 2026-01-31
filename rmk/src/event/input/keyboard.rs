use postcard::experimental::max_size::MaxSize;
use rmk_macro::input_event;
use serde::{Deserialize, Serialize};

use crate::input_device::rotary_encoder::Direction;

/// `KeyboardEvent` is the event whose `KeyAction` is stored in the keymap.
///
/// `KeyboardEvent` is different from events from pointing devices,
/// events from pointing devices are processed directly by the corresponding processors,
/// while `KeyboardEvent` is processed by the keyboard with the keymap.
#[input_event(channel_size = 16)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyboardEvent {
    pub pressed: bool,
    pub pos: KeyboardEventPos,
}

impl KeyboardEvent {
    pub fn key(row: u8, col: u8, pressed: bool) -> Self {
        Self {
            pressed,
            pos: KeyboardEventPos::Key(KeyPos { row, col }),
        }
    }

    pub fn rotary_encoder(id: u8, direction: Direction, pressed: bool) -> Self {
        Self {
            pressed,
            pos: KeyboardEventPos::RotaryEncoder(RotaryEncoderPos { id, direction }),
        }
    }
}

/// The position of the keyboard event.
///
/// The position can be either a key (row, col), or a rotary encoder (id, direction)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyboardEventPos {
    Key(KeyPos),
    RotaryEncoder(RotaryEncoderPos),
}

impl KeyboardEventPos {
    pub(crate) fn key_pos(col: u8, row: u8) -> Self {
        Self::Key(KeyPos { row, col })
    }

    pub(crate) fn rotary_encoder_pos(id: u8, direction: Direction) -> Self {
        Self::RotaryEncoder(RotaryEncoderPos { id, direction })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyPos {
    pub row: u8,
    pub col: u8,
}

/// Event for rotary encoder
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RotaryEncoderPos {
    /// The id of the rotary encoder
    pub id: u8,
    /// The direction of the rotary encoder
    pub direction: Direction,
}
