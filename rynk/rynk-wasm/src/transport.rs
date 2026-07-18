//! The JS-owned byte link as a [`RynkDevice`], plus the `rynk::io::Read`/`Write`
//! halves its `open()` hands out. The page owns the link's lifetime: it opens
//! the link before `connect` and closes it on teardown — nothing here closes it.

use js_sys::{Promise, Uint8Array};
use rynk::io::{ErrorKind, ErrorType, Read, Write};
use rynk::{RynkDevice, RynkHostError};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    /// JS byte link. `recv()` returns bytes, or an empty array only at EOF.
    #[derive(Clone)]
    pub type JsByteLink;

    #[wasm_bindgen(method, getter)]
    fn label(this: &JsByteLink) -> String;

    #[wasm_bindgen(method, catch)]
    async fn send(this: &JsByteLink, frame: Uint8Array) -> Result<(), JsValue>;

    /// A raw `Promise` import (not `async`) so the reader gets a nameable
    /// future it can park across cancelled `read`s.
    #[wasm_bindgen(method, catch)]
    fn recv(this: &JsByteLink) -> Result<Promise, JsValue>;
}

/// The browser owns discovery (WebSerial/WebHID chooser) and opens the link, so
/// the already-open [`JsByteLink`] itself is the transport's [`RynkDevice`]:
/// `open()` only wraps it into halves, and the trait's `connect()` handshakes.
impl RynkDevice for JsByteLink {
    type Read = WasmReader;
    type Write = WasmWriter;

    fn label(&self) -> String {
        self.label()
    }

    async fn open(self) -> Result<(WasmReader, WasmWriter), RynkHostError> {
        Ok((
            WasmReader {
                link: self.clone(),
                recv: None,
                pending: Vec::new(),
                pos: 0,
            },
            WasmWriter { link: self },
        ))
    }
}

/// Read half of the JS byte link, buffering `recv()` chunks.
pub struct WasmReader {
    link: JsByteLink,
    /// In-flight `recv()`, parked so a cancelled `read` resumes it.
    recv: Option<JsFuture>,
    /// Holds a chunk larger than one `read` buffer across reads.
    pending: Vec<u8>,
    pos: usize,
}

impl ErrorType for WasmReader {
    type Error = ErrorKind;
}

impl Read for WasmReader {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Refill once the current chunk is drained.
        while self.pos >= self.pending.len() {
            if self.recv.is_none() {
                self.recv = Some(JsFuture::from(self.link.recv().map_err(|_| ErrorKind::Other)?));
            }
            let value = self.recv.as_mut().unwrap().await;
            self.recv = None;
            let value = value.map_err(|_| ErrorKind::Other)?;
            // Only an empty byte array is EOF; any other JS value is invalid data.
            let chunk = value.dyn_into::<Uint8Array>().map_err(|_| ErrorKind::InvalidData)?;
            if chunk.length() == 0 {
                return Ok(0); // EOF
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

/// Write half of the JS byte link.
pub struct WasmWriter {
    link: JsByteLink,
}

impl ErrorType for WasmWriter {
    type Error = ErrorKind;
}

impl Write for WasmWriter {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
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
