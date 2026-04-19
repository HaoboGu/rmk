use core::sync::atomic::{AtomicU8, Ordering};

use rmk_types::connection::ConnectionType;

/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
pub(crate) static CONNECTION_STATE: AtomicU8 = AtomicU8::new(0);

/// Represents the current connection state of the device.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// No active connection (default state).
    Disconnected = 0x0,
    /// Connection is established and ready to use.
    Connected = 0x1,
    /// Connection exists but the device is suspended (e.g. USB suspend).
    Suspended = 0x2,
}

impl From<u8> for ConnectionState {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Disconnected,
            1 => Self::Connected,
            2 => Self::Suspended,
            _ => Self::Disconnected,
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
