//! wasm `Read`/`Write` transport over a JS-owned byte link.
//!
//! `wasm32` only. In the web deployment the **TS layer owns the device** — it
//! opens the port (WebSerial), runs the permanent `get_version` probe, and then
//! dynamically loads the version-matched `rynk` wasm. That wasm drives the
//! device through this transport: every [`Read`]/[`Write`] call is forwarded to
//! a TS-implemented [`JsLink`]. On any other target the crate compiles to an
//! empty library.
#![cfg(target_arch = "wasm32")]

use js_sys::Uint8Array;
use rynk::io::{ErrorKind, ErrorType, Read, Write};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// A TS-implemented byte link:
    /// `{ send(frame: Uint8Array): Promise<void>, recv(): Promise<Uint8Array> }`.
    /// `recv` resolves with the next chunk, or an empty array once the link is
    /// closed.
    pub type JsLink;

    #[wasm_bindgen(method, catch)]
    async fn send(this: &JsLink, frame: Uint8Array) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn recv(this: &JsLink) -> Result<JsValue, JsValue>;
}

/// A byte link backed by a TS-provided [`JsLink`], driving [`rynk::Client`].
///
/// `rynk` speaks `embedded_io_async` [`Read`]/[`Write`] over a raw byte stream,
/// while the JS link delivers whole chunks. [`pending`](Self::pending) holds the
/// remainder of a chunk that didn't fit the caller's `read` buffer, so frame
/// reassembly never drops over-read bytes.
pub struct BridgeTransport {
    link: JsLink,
    pending: Vec<u8>,
}

impl BridgeTransport {
    /// Wrap a TS-provided link. The link must already be connected (port open).
    pub fn new(link: JsLink) -> Self {
        Self {
            link,
            pending: Vec::new(),
        }
    }
}

impl ErrorType for BridgeTransport {
    type Error = ErrorKind;
}

impl Read for BridgeTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if self.pending.is_empty() {
            let value = self.link.recv().await.map_err(|_| ErrorKind::Other)?;
            let chunk: Uint8Array = value.unchecked_into();
            // An empty chunk marks the closed link; `rynk` maps `Ok(0)` to
            // `TransportError::Disconnected`.
            if chunk.length() == 0 {
                return Ok(0);
            }
            self.pending = chunk.to_vec();
        }
        let n = self.pending.len().min(buf.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

impl Write for BridgeTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.link
            .send(Uint8Array::from(buf))
            .await
            .map_err(|_| ErrorKind::Other)?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
