//! Split keyboard events

use rmk_macro::controller_event;

/// Peripheral connected state changed event
#[controller_event(channel_size = crate::PERIPHERAL_CONNECTED_EVENT_CHANNEL_SIZE, pubs = crate::PERIPHERAL_CONNECTED_EVENT_PUB_SIZE, subs = crate::PERIPHERAL_CONNECTED_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeripheralConnectedEvent {
    pub id: usize,
    pub connected: bool,
}

/// Connected to central state changed event
#[controller_event(channel_size = crate::CENTRAL_CONNECTED_EVENT_CHANNEL_SIZE, pubs = crate::CENTRAL_CONNECTED_EVENT_PUB_SIZE, subs = crate::CENTRAL_CONNECTED_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CentralConnectedEvent {
    pub connected: bool,
}

/// Peripheral battery level changed event
#[controller_event(channel_size = crate::PERIPHERAL_BATTERY_EVENT_CHANNEL_SIZE, pubs = crate::PERIPHERAL_BATTERY_EVENT_PUB_SIZE, subs = crate::PERIPHERAL_BATTERY_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeripheralBatteryEvent {
    pub id: usize,
    pub level: u8,
}

/// Clear BLE peer information event
#[cfg(feature = "_ble")]
#[controller_event(channel_size = crate::CLEAR_PEER_EVENT_CHANNEL_SIZE, pubs = crate::CLEAR_PEER_EVENT_PUB_SIZE, subs = crate::CLEAR_PEER_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ClearPeerEvent;
