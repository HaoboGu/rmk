//! Communication protocol definitions.
//!
//! RMK supports two host-communication protocols:
//!
//! - [`vial`] — Legacy Vial/Via protocol for compatibility with the Vial GUI.
//!   Always available.
//! - [`rmk`] — RMK native protocol built on postcard-rpc. Provides typed
//!   endpoints for keymap, combo, morse, fork, encoder, macro, and status
//!   queries over COBS-framed byte streams (USB bulk or BLE serial).
//!   Enabled by the `rmk_protocol` feature.
//!
//! The two protocols are mutually exclusive at the firmware level
//! (`rmk_protocol` and `vial` features cannot be enabled together).

#[cfg(feature = "rmk_protocol")]
pub mod rmk;
pub mod vial;
