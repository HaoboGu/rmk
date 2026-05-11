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

// `RynkBleTransport` stays crate-internal — its `Server` parameter is
// `pub(crate)`-only, and `BleTransport::with_rynk_service` provides the
// stable user-facing handle.
#[cfg(feature = "_ble")]
pub(crate) use ble::RynkBleTransport;
#[cfg(not(feature = "_no_usb"))]
pub use usb::RynkUsbTransport;
