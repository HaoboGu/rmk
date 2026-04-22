//! Host configurator support (keymap editing, firmware introspection, etc.).
//!
//! Organized along two axes:
//! - **Protocol** — Via/Vial (`via/`) or RMK/rynk (`rynk/`). One picks a
//!   protocol via the `vial` or `rmk_protocol` Cargo feature (mutually
//!   exclusive; enforced in `crate::lib`).
//! - **Transport** — how bytes reach the host: USB HID, BLE HID, USB bulk,
//!   BLE custom-serial. Implementations live in `transport/`.
//!
//! The [`HostService`] trait represents one `(protocol, transport)` pair's
//! run loop; [`HostRx`] / [`HostTx`] are the byte-level transport halves.
//! Call sites use the [`HostServiceImpl`] alias to stay protocol-agnostic.

// The `vial` / `rmk_protocol` mutual-exclusivity guard lives in `crate::lib.rs`.
#[cfg(all(feature = "host", not(any(feature = "vial", feature = "rmk_protocol"))))]
compile_error!(
    "Enabling the `host` feature requires selecting a protocol: enable either `vial` or `rmk_protocol`."
);

#[cfg(feature = "rmk_protocol")]
pub(crate) mod rynk;
#[cfg(feature = "storage")]
pub(crate) mod storage;
pub(crate) mod transport;
#[cfg(feature = "vial")]
pub(crate) mod via;

/// Errors that any host transport may surface.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum HostError {
    /// The underlying transport is not connected / has been torn down.
    Disconnected,
    /// An I/O error occurred while moving bytes.
    Io,
    /// The caller-provided recv buffer is too small to hold the incoming frame.
    BufferTooSmall,
    /// The caller-provided payload exceeds the transport's maximum frame size.
    FrameTooLarge,
    /// The underlying transport overflowed its own buffer before delivery.
    TransportOverflow,
}

/// Receive half of a host transport.
///
/// `recv(buf)` returns one complete protocol message. Framing is the
/// transport's responsibility — packet transports (USB HID, BLE HID) deliver
/// one report per call; byte-stream transports (USB bulk, BLE serial) apply
/// COBS internally and hand up one decoded frame per call. Each transport
/// documents its own required buffer size in its module.
pub(crate) trait HostRx {
    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, HostError>;
}

/// Send half of a host transport.
pub(crate) trait HostTx {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), HostError>;
}

/// A protocol-level host service. Drives its own receive/process/send loop.
///
/// One `(protocol, transport)` pair = one implementer. Because `rmk_protocol`
/// and `vial` are mutually exclusive (see `Cargo.toml`), at most one
/// implementer is live per firmware build.
pub(crate) trait HostService {
    async fn run(&mut self);
}

// The active-protocol service type is re-exported as `HostServiceImpl`.
// Call sites import one name and get the correct service type for whichever
// protocol is enabled. Construction is still per-protocol — Vial's factories
// take a `VialConfig`, rynk's take none — so call sites cfg-gate the
// constructor arguments. The two services are never in scope simultaneously
// (mutually exclusive features).

#[cfg(feature = "vial")]
pub(crate) use via::VialService as HostServiceImpl;

#[cfg(feature = "rmk_protocol")]
pub(crate) use rynk::RynkService as HostServiceImpl;
