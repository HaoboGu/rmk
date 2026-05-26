//! Shared error type for RMK-authored rynk transport adapters.
//!
//! Transports that wrap foreign byte streams already implement
//! [`embedded_io_async::Read`] / [`Write`](embedded_io_async::Write) and
//! carry their own error (USB CDC-ACM, UART). Transports where RMK
//! hand-writes the adapter — currently only BLE GATT — need an error type
//! that satisfies the [`embedded_io_async::Error`] bound, and they all want
//! the same thing: the framing layer only distinguishes "live" from "gone",
//! so every failure collapses to [`ErrorKind::ConnectionReset`].
//!
//! [`RynkService::run_session`](super::RynkService::run_session) discards the
//! error value entirely (it returns on any `Err`), so this type carries no
//! payload — it exists solely to name the closed-transport condition once.

use embedded_io_async::ErrorKind;

/// Error for RMK-authored rynk transport adapters. Always reports
/// [`ErrorKind::ConnectionReset`] — the framing layer only cares about live
/// vs. gone.
#[derive(Debug)]
pub struct RynkTransportError;

impl core::fmt::Display for RynkTransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("rynk transport closed")
    }
}

impl core::error::Error for RynkTransportError {}

impl embedded_io_async::Error for RynkTransportError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::ConnectionReset
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for RynkTransportError {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "RynkTransportError")
    }
}
