//! Shared connection type definitions used across RMK crates.

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// Connection type for the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectionType {
    Usb,
    Ble,
}

/// Unknown values default to [`ConnectionType::Usb`] for forward-compatibility:
/// if a newer firmware writes a variant this version doesn't recognise
/// (e.g. from stored settings), USB is the safest fallback.
impl From<u8> for ConnectionType {
    fn from(value: u8) -> Self {
        match value {
            0 => ConnectionType::Usb,
            1 => ConnectionType::Ble,
            _ => ConnectionType::Usb,
        }
    }
}

impl From<ConnectionType> for u8 {
    fn from(value: ConnectionType) -> Self {
        match value {
            ConnectionType::Usb => 0,
            ConnectionType::Ble => 1,
        }
    }
}
