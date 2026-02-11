//! Events for pointing devices
use postcard::experimental::max_size::MaxSize;
use rmk_macro::event;
use serde::{Deserialize, Serialize};

#[event(channel_size = crate::POINTING_EVENT_CHANNEL_SIZE)]
#[derive(Serialize, Deserialize, Clone, Debug, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PointingEvent(pub [AxisEvent; 3]);

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

/// Set the CPI (Resolution) of the pointing device
/// TODO: Make the channel size configurable
#[event(channel_size = 8, pubs = 2, subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PointingSetCpiEvent {
    pub device_id: u8,
    pub cpi: u16,
}
