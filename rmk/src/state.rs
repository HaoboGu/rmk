use core::sync::atomic::{AtomicU8, Ordering};

use rmk_types::connection::ConnectionType;

/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
pub(crate) static CONNECTION_STATE: AtomicU8 = AtomicU8::new(0);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected = 0,
    Connected = 1,
    Suspended = 2,
}

impl From<u8> for ConnectionState {
    fn from(value: u8) -> Self {
        match value {
            1 => ConnectionState::Connected,
            2 => ConnectionState::Suspended,
            _ => ConnectionState::Disconnected,
        }
    }
}

impl From<ConnectionState> for u8 {
    fn from(state: ConnectionState) -> u8 {
        state as u8
    }
}

pub fn get_connection_type() -> ConnectionType {
    CONNECTION_TYPE.load(Ordering::Acquire).into()
}

pub fn get_connection_state() -> ConnectionState {
    CONNECTION_STATE.load(Ordering::Acquire).into()
}
