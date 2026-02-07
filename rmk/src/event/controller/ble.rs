//! BLE connection events

use rmk_macro::controller_event;

use crate::ble::BleState;

/// BLE state changed event
#[controller_event(channel_size = crate::BLE_STATE_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::BLE_STATE_CHANGE_EVENT_PUB_SIZE, subs = crate::BLE_STATE_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleStateChangeEvent {
    pub profile: u8,
    pub state: BleState,
}

impl BleStateChangeEvent {
    pub fn new(profile: u8, state: BleState) -> Self {
        Self { profile, state }
    }
}

/// BLE profile changed event
#[controller_event(channel_size = crate::BLE_PROFILE_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::BLE_PROFILE_CHANGE_EVENT_PUB_SIZE, subs = crate::BLE_PROFILE_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleProfileChangeEvent {
    pub profile: u8,
}
