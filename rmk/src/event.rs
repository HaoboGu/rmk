use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::input_device::rotary_encoder::Direction;
#[cfg(feature = "controller")]
use crate::keycode::ModifierCombination;

/// Raw events from input devices and keyboards
///
/// This should be as close to the raw output of the devices as possible.
/// The input processors receives it, processes it,
/// and then converts it to the final keyboard/mouse report.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Event {
    /// Keyboard event
    Key(KeyEvent),
    /// Rotary encoder, ec11 compatible models
    RotaryEncoder(RotaryEncoderEvent),
    /// Multi-touch touchpad
    Touchpad(TouchpadEvent),
    /// Joystick, suppose we have x,y,z axes for this joystick
    Joystick([AxisEvent; 3]),
    /// An AxisEvent in a stream of events. The receiver should keep receiving events until it receives [`Event::Eos`] event.
    AxisEventStream(AxisEvent),
    /// Battery percentage event
    Battery(u16),
    /// Charging state changed event, true means charging, false means not charging
    ChargingState(bool),
    /// End of the event sequence
    ///
    /// This is used with [`Event::AxisEventStream`] to indicate the end of the event sequence.
    Eos,
    /// Custom event
    Custom([u8; 16]),
}

/// Event for rotary encoder
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RotaryEncoderEvent {
    /// The id of the rotary encoder
    pub id: u8,
    /// The direction of the rotary encoder
    pub direction: Direction,
}

/// Event for multi-touch touchpad
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TouchpadEvent {
    /// Finger slot
    pub finger: u8,
    /// X, Y, Z axes for touchpad
    pub axis: [AxisEvent; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AxisEvent {
    /// The axis event value type, relative or absolute
    pub typ: AxisValType,
    /// The axis name
    pub axis: Axis,
    /// Value of the axis event
    pub value: i16,
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AxisValType {
    /// The axis value is relative
    Rel,
    /// The axis value is absolute
    Abs,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Axis {
    X,
    Y,
    Z,
    H,
    V,
    // .. More is allowed
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyEvent {
    pub id: KeyId,
    pub pressed: bool,
}

impl KeyEvent {
    pub fn key(col: u8, row: u8, pressed: bool) -> Self {
        Self {
            id: KeyId::Key(KeyPos { row, col }),
            pressed,
        }
    }

    pub fn rotary_encoder(id: u8, direction: Direction) -> Self {
        Self {
            id: KeyId::RotaryEncoder(id, direction),
            pressed: true,
        }
    }
}

impl KeyEvent {
    /// Get the row if this is a Key variant, otherwise panic
    pub fn row(&self) -> u8 {
        self.id.row().expect("KeyEvent::row() called on non-Key variant")
    }

    /// Get the column if this is a Key variant, otherwise panic
    pub fn col(&self) -> u8 {
        self.id.col().expect("KeyEvent::col() called on non-Key variant")
    }

    /// Check if this is a Key variant
    pub fn is_key(&self) -> bool {
        self.id.is_key()
    }

    /// Check if this is a RotaryEncoder variant
    pub fn is_rotary_encoder(&self) -> bool {
        self.id.is_rotary_encoder()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyPos {
    pub row: u8,
    pub col: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyId {
    Key(KeyPos),
    RotaryEncoder(u8, Direction),
}

impl From<KeyId> for usize {
    /// Convert KeyId to a unique usize representation
    ///
    /// Encoding scheme:
    /// - Key(KeyPos): 0..65536 (row * 256 + col)
    /// - RotaryEncoder(u8, Direction): 65536.. (65536 + encoder_id * 3 + direction)
    fn from(key_id: KeyId) -> Self {
        match key_id {
            KeyId::Key(KeyPos { row, col }) => (row as usize) * 256 + (col as usize),
            KeyId::RotaryEncoder(encoder_id, direction) => {
                let direction_value = match direction {
                    Direction::Clockwise => 0,
                    Direction::CounterClockwise => 1,
                    Direction::None => 2,
                };
                65536 + (encoder_id as usize) * 3 + direction_value
            }
        }
    }
}

impl KeyId {
    /// Get the row if this is a Key variant, otherwise return None
    pub fn row(&self) -> Option<u8> {
        match self {
            KeyId::Key(KeyPos { row, .. }) => Some(*row),
            KeyId::RotaryEncoder(_, _) => None,
        }
    }

    /// Get the column if this is a Key variant, otherwise return None
    pub fn col(&self) -> Option<u8> {
        match self {
            KeyId::Key(KeyPos { col, .. }) => Some(*col),
            KeyId::RotaryEncoder(_, _) => None,
        }
    }

    /// Check if this is a Key variant
    pub fn is_key(&self) -> bool {
        matches!(self, KeyId::Key(_))
    }

    /// Check if this is a RotaryEncoder variant
    pub fn is_rotary_encoder(&self) -> bool {
        matches!(self, KeyId::RotaryEncoder(_, _))
    }
}

/// Event for controllers
#[cfg(feature = "controller")]
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ControllerEvent {
    /// Key event and action
    Key(KeyEvent, KeyAction),
    /// Battery percent changed
    Battery(u16),
    /// Charging state changed
    ChargingState(bool),
    /// Ble profile changed
    BleProfile(u8),
    /// Layer changed
    Layer(u8),
    /// Modifier changed
    Modifier(ModifierCombination),
    /// Typing speed
    Wpm(u16),
    /// Usb or Ble connection
    ConnectionType(u8),
    /// Split peripheral connection
    SplitPeripheral(usize, bool),
}
