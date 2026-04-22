//! Vial HID transports (fixed 32-byte reports).
//!
//! Each submodule provides a type that implements
//! `crate::hid::HidReaderTrait<ReportType = ViaReport>` and
//! `crate::hid::HidWriterTrait<ReportType = ViaReport>`, so `VialService`
//! plugs in using the same trait pair the rest of the crate already uses
//! for keyboard/LED reports — no Vial-local transport trait needed.
//!
//! - `usb_hid` — USB HID class, fixed 32-byte reports.
//! - `ble_hid` — BLE HID custom characteristic, fixed 32-byte reports.
//!
//! rynk doesn't use these — its transports impl
//! `postcard_rpc::server::{WireRx, WireTx}` directly.

#[cfg(feature = "_ble")]
pub(crate) mod ble_hid;
#[cfg(not(feature = "_no_usb"))]
pub(crate) mod usb_hid;
