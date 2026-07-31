//! Shared error type for RMK host-protocol transport adapters.

use embedded_io_async::ErrorKind;

#[derive(Debug)]
pub(crate) struct HostTransportError;

impl core::fmt::Display for HostTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("host transport closed")
    }
}

impl core::error::Error for HostTransportError {}

impl embedded_io_async::Error for HostTransportError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::ConnectionReset
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for HostTransportError {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "HostTransportError")
    }
}
