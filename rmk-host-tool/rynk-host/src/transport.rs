//! [`Transport`] trait + concrete implementations.
//!
//! A [`Transport`] is the lowest-level handle the host needs to talk to a
//! firmware running Rynk. Each call sends one request frame and waits for
//! the matching response frame (correlated by SEQ). Topic frames arriving
//! while the call is in flight are pushed onto the [`Transport::topics`]
//! broadcast for any subscriber.

use rmk_types::protocol::rynk::Cmd;
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::sync::broadcast;

/// A raw topic frame: the [`Cmd`] tag and its postcard-encoded payload.
/// Subscribers decode the payload themselves so the broadcast channel
/// stays type-erased.
#[derive(Debug, Clone)]
pub struct TopicFrame {
    pub cmd: Cmd,
    pub payload: Vec<u8>,
}

/// Errors a [`Transport`] can return.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport disconnected")]
    Disconnected,
    #[error("io error: {0}")]
    Io(String),
    #[error("postcard serialize: {0}")]
    Serialize(postcard::Error),
    #[error("postcard deserialize: {0}")]
    Deserialize(postcard::Error),
    #[error("response cmd mismatch: sent {sent:?}, got {got:?}")]
    CmdMismatch { sent: Cmd, got: Cmd },
    #[error("response sequence mismatch: sent {sent}, got {got}")]
    SeqMismatch { sent: u8, got: u8 },
    #[error("frame too long: {len} > {max}")]
    FrameTooLong { len: usize, max: usize },
    #[error("response timeout")]
    Timeout,
    #[error("device not found: {0}")]
    DeviceNotFound(String),
}

/// Per-host transport handle.
///
/// `request` is async — `UsbBulkTransport` calls into `nusb` futures and
/// `BleGattTransport` calls into `btleplug` futures. Both run on a tokio
/// runtime.
///
/// `topics` hands out a fresh subscriber for each caller; topic frames
/// are broadcast to all live subscribers (lagged receivers are dropped
/// per `tokio::sync::broadcast` semantics).
pub trait Transport: Send {
    fn request<Req: Serialize + Send + Sync, Resp: DeserializeOwned + Send>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> impl core::future::Future<Output = Result<Resp, TransportError>> + Send;

    fn topics(&self) -> broadcast::Receiver<TopicFrame>;
}
