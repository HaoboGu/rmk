//! rynk transports (COBS-framed postcard bytes).
//!
//! Each submodule impls `postcard_rpc::server::{WireRx, WireTx}` directly,
//! so `define_dispatch!` consumes the transport without an adapter layer.
//!
//! - `usb_bulk`    — USB bulk endpoint pair with COBS framing.
//! - `ble_serial`  — Custom BLE GATT service with COBS framing.
//!
//! postcard-rpc ships a ready-made `embassy_usb_v0_5` impl, but RMK uses
//! embassy-usb 0.6, so these are hand-written.

#[cfg(feature = "_ble")]
pub(crate) mod ble_serial;
#[cfg(not(feature = "_no_usb"))]
pub(crate) mod usb_bulk;
