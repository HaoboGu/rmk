//! Communication protocol definitions.
//!
//! This module contains the protocol, type definitions and constants for communicating with
//! keyboard configuration software like Vial.
//!
//! - `vial` - Vial protocol implementation
//! - `rmk` - RMK native protocol ICD (enabled by the `rmk_protocol` feature)

#[cfg(feature = "rmk_protocol")]
pub mod rmk;
pub mod vial;
