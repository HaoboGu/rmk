#![cfg(feature = "split")]
//! Split keyboard events

use rmk_macro::controller_event;

/// Peripheral connection state changed event
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeripheralConnectionEvent {
    pub id: usize,
    pub connected: bool,
}

/// Peripheral battery level changed event
#[controller_event(channel_size = 2, subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeripheralBatteryEvent {
    pub id: usize,
    pub level: u8,
}

/// Central connection state changed event
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CentralConnectionEvent {
    pub connected: bool,
}

/// Clear BLE peer information event
#[cfg(feature = "_ble")]
#[controller_event(channel_size = 1, subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ClearPeerEvent;
