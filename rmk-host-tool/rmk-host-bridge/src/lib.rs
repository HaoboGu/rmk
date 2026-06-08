//! wasm `Transport` over a JS-owned byte link.
//!
//! `wasm32` only. In the web deployment the **TS layer owns the device** — it
//! opens the port (WebSerial), runs the permanent `get_version` probe, and then
//! dynamically loads the version-matched `rynk-core` wasm. That wasm core talks
//! to the device through this transport: every [`Transport::send`]/`recv` is
//! forwarded to a TS-implemented [`JsLink`]. On any other target the crate
//! compiles to an empty library.
#![cfg(target_arch = "wasm32")]

use js_sys::Uint8Array;
use rmk_host::{Transport, TransportError};
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

/// A [`Transport`] backed by a TS-provided [`JsLink`].
pub struct BridgeTransport {
    link: JsLink,
}

impl BridgeTransport {
    /// Wrap a TS-provided link. The link must already be connected (port open).
    pub fn new(link: JsLink) -> Self {
        Self { link }
    }
}

impl Transport for BridgeTransport {
    async fn send(&mut self, frame: &[u8]) -> Result<(), TransportError> {
        self.link.send(Uint8Array::from(frame)).await.map_err(js_err)
    }

    async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        let value = self.link.recv().await.map_err(js_err)?;
        let chunk: Uint8Array = value.unchecked_into();
        if chunk.length() == 0 {
            return Err(TransportError::Disconnected);
        }
        Ok(chunk.to_vec())
    }
}

fn js_err(e: JsValue) -> TransportError {
    TransportError::Io(format!("{e:?}"))
}
