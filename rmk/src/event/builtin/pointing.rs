//! Control events for pointing devices

use rmk_macro::controller_event;

/// Set the CPI (Resolution) of the pointing device
#[controller_event(channel_size = crate::POINTING_CONTROL_SET_CPI_EVENT_CHANNEL_SIZE, pubs = crate::POINTING_CONTROL_SET_CPI_EVENT_PUB_SIZE, subs = crate::POINTING_CONTROL_SET_CPI_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PointingSetCpiEvent {
    pub device_id: u8,
    pub cpi: u16,
}

/// Set the rotational transform angle of the pointing device
#[controller_event(channel_size = crate::POINTING_CONTROL_ANGLE_EVENT_CHANNEL_SIZE, pubs = crate::POINTING_CONTROL_ANGLE_EVENT_PUB_SIZE, subs = crate::POINTING_CONTROL_ANGLE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PointingSetRotTransAngleEvent {
    pub device_id: u8,
    pub angle: i8,
}

/// Set the liftoff distance of the pointing device
#[controller_event(channel_size = crate::POINTING_CONTROL_SET_LO_EVENT_CHANNEL_SIZE, pubs = crate::POINTING_CONTROL_SET_LO_EVENT_PUB_SIZE, subs = crate::POINTING_CONTROL_SET_LO_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PointingSetLiftoffDistEvent {
    pub device_id: u8,
    pub distance: u8,
}

#[controller_event(channel_size = crate::POINTING_CONTROL_FORCE_AWAKE_EVENT_CHANNEL_SIZE, pubs = crate::POINTING_CONTROL_FORCE_AWAKE_EVENT_PUB_SIZE, subs = crate::POINTING_CONTROL_FORCE_AWAKE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PointingSetForceAwakeEvent {
    pub device_id: u8,
    pub force_awake: bool,
}
