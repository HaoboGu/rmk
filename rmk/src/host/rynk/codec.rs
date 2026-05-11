//! Transport-trait surface shared by every Rynk handler.
//!
//! `Header` itself (the wire-format struct + encode/decode) lives in
//! `rmk_types::protocol::rynk::header` so the host crate and the firmware
//! use the same implementation.

/// Errors a transport can surface to the dispatch loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireErr {
    /// Frame longer than the configured buffer.
    Overflow,
    /// Peer closed the connection (USB unconfigured / BLE disconnected).
    ConnectionClosed,
    /// Underlying transport reported an error other than disconnect.
    Io,
}

/// Transport TX half — writes one fully assembled frame to the wire.
///
/// The transport is responsible for any wire-level framing the medium
/// requires (USB short-packet / ZLP convention, BLE MTU chunking). The
/// dispatcher hands it a complete `[header_bytes, payload_bytes]`
/// buffer and expects the call to return only after the bytes have
/// been accepted.
pub trait WireTx {
    async fn send(&mut self, frame: &[u8]) -> Result<(), WireErr>;
}

/// Transport RX half — yields one complete frame at a time.
///
/// Implementations accumulate bytes from the medium until they have
/// `5 + LEN` bytes (header + payload), then return a slice of `buf`
/// covering those bytes. Bytes belonging to the next frame stay
/// buffered inside the transport for the next call.
pub trait WireRx {
    async fn recv<'b>(&mut self, buf: &'b mut [u8]) -> Result<&'b [u8], WireErr>;
}
