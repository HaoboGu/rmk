use core::sync::atomic::{AtomicU8, Ordering};

/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
pub(crate) static CONNECTION_STATE: AtomicU8 = AtomicU8::new(0);

/// Current default connection type
pub enum ConnectionType {
    Usb = 0,
    Ble = 1,
}

pub enum ConnectionState {
    Disconnected = 0,
    Connected = 1,
}

pub fn get_connection_type() -> ConnectionType {
    match CONNECTION_TYPE.load(Ordering::Acquire) as u8 {
        0 => ConnectionType::Usb,
        1 => ConnectionType::Ble,
        _ => unreachable!("Invalid connection type"),
    }
}

pub fn get_connection_state() -> ConnectionState {
    match CONNECTION_STATE.load(Ordering::Acquire) as u8 {
        0 => ConnectionState::Disconnected,
        1 => ConnectionState::Connected,
        _ => unreachable!("Invalid connection state"),
    }
}
