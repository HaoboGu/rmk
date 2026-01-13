#![cfg(feature = "_ble")]
//! BLE connection events

use rmk_macro::controller_event;

use crate::ble::BleState;

/// BLE state changed event
#[controller_event(channel_size = 2, subs = 2)]
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
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BleProfileChangeEvent {
    pub profile: u8,
}
