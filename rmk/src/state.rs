use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use rmk_types::connection::ConnectionType;

/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
pub(crate) static CONNECTION_STATE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connected,
}

impl ConnectionState {
    pub(crate) fn from_atomic(state: &AtomicBool) -> Self {
        if state.load(Ordering::Acquire) {
            Self::Connected
        } else {
            Self::Disconnected
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
