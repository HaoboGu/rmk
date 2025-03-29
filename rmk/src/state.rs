use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
pub(crate) static CONNECTION_STATE: AtomicBool = AtomicBool::new(false);

/// Current default connection type
pub enum ConnectionType {
    Usb = 0,
    Ble = 1,
}

pub enum ConnectionState {
    Disconnected,
    Connected,
}

impl From<u8> for ConnectionType {
    fn from(value: u8) -> Self {
        match value {
            0 => ConnectionType::Usb,
            1 => ConnectionType::Ble,
            _ => unreachable!("Invalid connection type"),
        }
    }
}

impl From<ConnectionType> for u8 {
    fn from(conn_type: ConnectionType) -> u8 {
        match conn_type {
            ConnectionType::Usb => 0,
            ConnectionType::Ble => 1,
        }
    }
}

pub fn get_connection_type() -> ConnectionType {
    CONNECTION_TYPE.load(Ordering::Acquire).into()
}

pub fn get_connection_state() -> ConnectionState {
    CONNECTION_STATE.load(Ordering::Acquire).into()
}

impl From<bool> for ConnectionState {
    fn from(value: bool) -> Self {
        if value {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        }
    }
}

impl From<ConnectionState> for bool {
    fn from(state: ConnectionState) -> bool {
        match state {
            ConnectionState::Connected => true,
            ConnectionState::Disconnected => false,
        }
    }
}
