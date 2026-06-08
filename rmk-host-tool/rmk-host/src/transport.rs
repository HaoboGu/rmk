//! Transport trait and shared transport errors.
//!
//! This module is the narrow byte-link seam a transport crate implements: just
//! [`Transport`], [`TransportError`], and [`MaybeSend`]. Request-layer types
//! (`RequestError`, `TopicFrame`) live in [`crate::client`] with the protocol
//! logic that owns them.

use thiserror::Error;

/// A byte link to a Rynk device.
pub trait Transport: MaybeSend {
    /// Send one complete protocol frame.
    fn send(&mut self, frame: &[u8]) -> impl core::future::Future<Output = Result<(), TransportError>> + MaybeSend;

    /// Receive the next byte chunk.
    fn recv(&mut self) -> impl core::future::Future<Output = Result<Vec<u8>, TransportError>> + MaybeSend;
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

/// `Send` on native targets, no-op on `wasm32`.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + ?Sized> MaybeSend for T {}
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSend for T {}
