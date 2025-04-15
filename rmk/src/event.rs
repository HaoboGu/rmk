use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::input_device::rotary_encoder::Direction;

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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyEvent {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
}
