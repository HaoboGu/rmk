//! Per-transport adapters for the Rynk service.
//!
//! Each transport owns its endpoints/GATT characteristics and a single
//! `run(&service)` future. The future is joined into the existing
//! `::rmk::run_all!(…)` chain by macro-generated entry-point code, exactly
//! as Vial's `host_service.run()` is today.
//!
//! Invariants:
//! - One future per transport (no spawned tasks → no `'static` plumbing).
//! - No channels between transport and service; the transport calls
//!   [`RynkService::dispatch`](super::RynkService::dispatch) inline with a
//!   stack-resident frame buffer.

#[cfg(feature = "_ble")]
pub(crate) mod ble;
#[cfg(not(feature = "_no_usb"))]
pub mod usb;

// The BLE side has no public transport handle — `BleTransport::with_rynk_service`
// is the stable entry point, and `ble::run_ble_rynk` is the per-connection
// runner it dispatches to.
#[cfg(feature = "_ble")]
pub(crate) use ble::run_ble_rynk;
#[cfg(not(feature = "_no_usb"))]
pub use usb::RynkUsbTransport;
