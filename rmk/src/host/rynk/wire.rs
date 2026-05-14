//! Wire-level transport trait surface used by every Rynk transport.
//!
//! The message layout itself (header accessors, payload sub-slice) lives
//! in [`rmk_types::protocol::rynk::RynkMessage`]; this module only
//! defines how a transport pushes assembled bytes onto / pulls them off
//! the wire.

/// Errors a transport can surface to the dispatch loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireErr {
    /// Message longer than the configured buffer.
    Overflow,
    /// Peer closed the connection (USB unconfigured / BLE disconnected).
    ConnectionClosed,
    /// Underlying transport reported an error other than disconnect.
    Io,
}

/// Transport TX half — writes one fully assembled message to the wire.
///
/// The transport is responsible for any wire-level framing the medium
/// requires (USB short-packet / ZLP convention, BLE MTU chunking). The
/// dispatcher hands it a complete `[header_bytes, payload_bytes]`
/// buffer and expects the call to return only after the bytes have
/// been accepted.
pub trait WireTx {
    async fn send(&mut self, msg: &[u8]) -> Result<(), WireErr>;
}

/// Transport RX half — yields one complete message at a time.
///
/// Implementations accumulate bytes from the medium until they have
/// `5 + LEN` bytes (header + payload), then return a slice of `buf`
/// covering those bytes. Bytes belonging to the next message stay
/// buffered inside the transport for the next call.
pub trait WireRx {
    async fn recv<'b>(&mut self, buf: &'b mut [u8]) -> Result<&'b [u8], WireErr>;
}
