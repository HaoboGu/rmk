//! Connection related types

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectionType {
    Usb,
    Ble,
}

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
