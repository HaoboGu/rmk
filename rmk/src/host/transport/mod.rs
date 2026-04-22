//! Transport implementations for host services.
//!
//! Each submodule provides a `HostRx` / `HostTx` implementation for one wire:
//! - `usb_hid`     — USB HID class, fixed 32-byte reports (Vial)
//! - `ble_hid`     — BLE HID custom characteristic, fixed 32-byte reports (Vial)
//! - `usb_bulk`    — USB bulk endpoint pair with COBS framing (rynk)
//! - `ble_serial`  — Custom BLE GATT service with COBS framing (rynk)
//!
//! The umbrella `HostRx` / `HostTx` traits live in `rmk/src/host/mod.rs`;
//! per the project rule, we do not `pub use` submodule items here.

#[cfg(all(feature = "vial", feature = "_ble"))]
pub(crate) mod ble_hid;
#[cfg(all(feature = "rmk_protocol", feature = "_ble"))]
pub(crate) mod ble_serial;
#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
pub(crate) mod usb_hid;
#[cfg(all(feature = "rmk_protocol", not(feature = "_no_usb")))]
pub(crate) mod usb_bulk;
