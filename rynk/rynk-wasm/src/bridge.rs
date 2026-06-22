//! `Read`/`Write` transport over a JS-owned byte link.
//!
//! In the web deployment the **page owns the device** — it opens the port
//! (WebSerial), runs the permanent `get_version` probe, then loads this wasm.
//! The `Client` drives the device through this transport: every [`Read`]/[`Write`]
//! call is forwarded to a JS-implemented [`JsLink`].

use js_sys::Uint8Array;
use rynk::io::{ErrorKind, ErrorType, Read, Write};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// A JS-implemented byte link:
    /// `{ send(frame: Uint8Array): Promise<void>, recv(): Promise<Uint8Array> }`.
    /// `recv` resolves with the next non-empty chunk, or an empty array *only*
    /// once the link is closed — an empty chunk is read as EOF, not a short read.
    pub type JsLink;

    #[wasm_bindgen(method, catch)]
    async fn send(this: &JsLink, frame: Uint8Array) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn recv(this: &JsLink) -> Result<JsValue, JsValue>;
}

/// A byte link backed by a JS-provided [`JsLink`], driving [`rynk::Client`].
///
/// `rynk` speaks `embedded_io_async` [`Read`]/[`Write`] over a raw byte stream
/// while the JS link delivers whole chunks; `pending`/`pos` hold a chunk's
/// over-read remainder across reads (the same shape as `rynk-ble`'s transport),
/// so frame reassembly never drops over-read bytes.
///
/// `read` is not cancel-safe — a JS promise can't be cancelled, so a dropped
/// `recv` future loses its chunk. The session API drives one request to
/// completion at a time and never cancels a read, so the `rynk` contract holds.
pub struct BridgeTransport {
    link: JsLink,
    pending: Vec<u8>,
    pos: usize,
}

impl BridgeTransport {
    /// Wrap a JS-provided link. The link must already be connected (port open).
    pub fn new(link: JsLink) -> Self {
        Self {
            link,
            pending: Vec::new(),
            pos: 0,
        }
    }
}

impl ErrorType for BridgeTransport {
    type Error = ErrorKind;
}

impl Read for BridgeTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        while self.pos >= self.pending.len() {
            let value = self.link.recv().await.map_err(|_| ErrorKind::Other)?;
            // `dyn_into` keeps a non-`Uint8Array` reply (a buggy link) a clean
            // transport error rather than a wasm trap from an unchecked cast.
            let chunk: Uint8Array = value.dyn_into().map_err(|_| ErrorKind::Other)?;
            // An empty chunk marks the closed link; `rynk` maps `Ok(0)` to
            // `TransportError::Disconnected`.
            if chunk.length() == 0 {
                return Ok(0);
            }
            self.pending = chunk.to_vec();
            self.pos = 0;
        }
        let n = buf.len().min(self.pending.len() - self.pos);
        buf[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
        self.pos += n;
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
