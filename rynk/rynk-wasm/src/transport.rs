//! `WasmTransport` adapts a JS-owned byte link to `rynk::io::Read`/`Write`.

use futures_channel::mpsc::{self, UnboundedReceiver};
use futures_util::StreamExt;
use futures_util::future::{AbortHandle, Abortable};
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

/// Buffered transport; a pump owns `recv()` so `read()` is cancel-safe.
pub struct WasmTransport {
    link: JsByteLink,
    /// Stops the pump task when the transport drops.
    pump: AbortHandle,
    rx: UnboundedReceiver<Vec<u8>>,
    pending: Vec<u8>,
    pos: usize,
}

impl WasmTransport {
    /// Start the receive pump for an already-open link.
    pub fn new(link: JsByteLink) -> Self {
        let (tx, rx) = mpsc::unbounded();
        // Clone the JsValue handle and cast it back to JsByteLink.
        let read_link: JsByteLink = link.clone().unchecked_into();
        // Dropping tx makes the next read report EOF.
        let (pump, registration) = AbortHandle::new_pair();
        spawn_local(async move {
            let _ = Abortable::new(
                async move {
                    while let Ok(value) = read_link.recv().await {
                        let Ok(chunk) = value.dyn_into::<Uint8Array>() else {
                            break; // invalid link implementation
                        };
                        if chunk.length() == 0 {
                            break; // EOF
                        }
                        if tx.unbounded_send(chunk.to_vec()).is_err() {
                            break; // transport dropped
                        }
                    }
                },
                registration,
            )
            .await;
        });
        Self {
            link,
            pump,
            rx,
            pending: Vec::new(),
            pos: 0,
        }
    }
}

impl Drop for WasmTransport {
    /// Abort the pump and close the JS link.
    fn drop(&mut self) {
        self.pump.abort();
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
        // Refill from the pump once the current chunk is drained.
        while self.pos >= self.pending.len() {
            // Closed channel = EOF; `rynk` maps `Ok(0)` to `Disconnected`.
            let Some(chunk) = self.rx.next().await else {
                return Ok(0);
            };
            self.pending = chunk;
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
