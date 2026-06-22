//! `rynk` compiled to a wasm package: the [`Client`](rynk::Client) plus the
//! in-crate JS-bridge transport (the [`bridge`] module) and a flat, interactive
//! [`session`] API exposed to JavaScript via `wasm-bindgen`.
//!
//! The caller owns the byte link: the page opens the port (WebSerial), runs the
//! permanent `get_version` probe, loads the version-matched build, then drives the
//! typed session through [`session::connect`] and the per-command functions —
//! over a JS bridge ([`BridgeTransport`](bridge::BridgeTransport)). The `Client`
//! itself never crosses into JS; it lives in a Rust-side session slot (see
//! [`session`]), mirroring the surface a native `rynk` consumer drives directly.
//!
//! BLE configuration stays native-only (the `rynk-ble` transport).
//!
//! Gated on `wasm32` — on any other target it compiles to an empty library.
//! Build with `wasm-pack build --target web`.
#![cfg(target_arch = "wasm32")]

pub mod bridge;
pub mod session;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}
