//! `WasmTransport` adapts a JS-owned byte link to `rynk::io::Read`/`Write`.

use std::future::Future;
use std::pin::Pin;

use js_sys::Uint8Array;
use rynk::io::{ErrorKind, ErrorType, Read, Write};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
extern "C" {
    /// JS byte link. `recv()` returns bytes, or an empty array only at EOF.
    pub type JsByteLink;

    #[wasm_bindgen(method, catch)]
    async fn send(this: &JsByteLink, frame: Uint8Array) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn recv(this: &JsByteLink) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn close(this: &JsByteLink) -> Result<(), JsValue>;
}

/// One in-flight `recv()` call, boxed so it can be parked in the transport.
type RecvFuture = Pin<Box<dyn Future<Output = Result<JsValue, JsValue>>>>;

/// Buffered transport over a JS byte link.
pub struct WasmTransport {
    link: JsByteLink,
    label: String,
    recv: Option<RecvFuture>,
    pending: Vec<u8>,
    pos: usize,
}

impl WasmTransport {
    /// Wrap an already-open link labeled with the page's device name.
    pub fn new(link: JsByteLink, label: String) -> Self {
        Self {
            link,
            label,
            recv: None,
            pending: Vec::new(),
            pos: 0,
        }
    }

    /// The display name the page supplied for this device.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Drop for WasmTransport {
    /// Close the JS link.
    fn drop(&mut self) {
        let link: JsByteLink = self.link.clone().unchecked_into();
        spawn_local(async move {
            let _ = link.close().await;
        });
    }
}

impl ErrorType for WasmTransport {
    type Error = ErrorKind;
}

impl Read for WasmTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Refill once the current chunk is drained.
        while self.pos >= self.pending.len() {
            if self.recv.is_none() {
                // Clone the handle into the future so it owns all it borrows.
                let link: JsByteLink = self.link.clone().unchecked_into();
                self.recv = Some(Box::pin(async move { link.recv().await }));
            }
            // Poll in place: a cancelled read() leaves the future parked in `self`.
            let value = self.recv.as_mut().unwrap().await.map_err(|_| ErrorKind::Other)?;
            self.recv = None;
            let Ok(chunk) = value.dyn_into::<Uint8Array>() else {
                return Ok(0); // invalid link implementation; `rynk` maps Ok(0) to Disconnected
            };
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

impl Write for WasmTransport {
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
