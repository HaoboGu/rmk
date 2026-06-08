//! Transport trait and shared transport errors.

use rmk_types::protocol::rynk::Cmd;
use thiserror::Error;

/// A byte link to a Rynk device.
pub trait Transport: MaybeSend {
    /// Send one complete protocol frame.
    fn send(&mut self, frame: &[u8]) -> impl core::future::Future<Output = Result<(), TransportError>> + MaybeSend;

    /// Receive the next byte chunk.
    fn recv(&mut self) -> impl core::future::Future<Output = Result<Vec<u8>, TransportError>> + MaybeSend;
}

/// A raw topic frame.
#[derive(Debug, Clone)]
pub struct TopicFrame {
    pub cmd: Cmd,
    pub payload: Vec<u8>,
}

/// Transport setup and I/O errors.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport disconnected")]
    Disconnected,
    #[error("io error: {0}")]
    Io(String),
    #[error("device not found: {0}")]
    DeviceNotFound(String),
}

/// Errors from one request round trip.
#[derive(Debug, Error)]
pub enum RequestError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The firmware accepted the request but answered with an error.
    #[error("device rejected {0}")]
    Rejected(#[from] rmk_types::protocol::rynk::RynkError),
    #[error("request encode failed for {0:?} (request exceeds tx buffer?)")]
    Encode(Cmd),
    #[error("response decode failed for {cmd:?}: {source}")]
    Deserialize { cmd: Cmd, source: postcard::Error },
    #[error("response for {cmd:?} had trailing bytes")]
    TrailingBytes { cmd: Cmd },
    #[error("response cmd mismatch: sent {sent:?}, got {got:?}")]
    CmdMismatch { sent: Cmd, got: Cmd },
    /// A topic-range `Cmd` was passed to a request method — topics are
    /// server→host push only.
    #[error("{0:?} is a topic, not a request")]
    TopicCmd(Cmd),
}

/// `Send` on native targets, no-op on `wasm32`.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + ?Sized> MaybeSend for T {}
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSend for T {}
