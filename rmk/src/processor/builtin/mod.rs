//! Built-in processor module for RMK
//!
//! This module contains built-in processor implementations for output devices.

#[cfg(feature = "_ble")]
pub mod battery_led;
pub mod led_indicator;
pub(crate) mod wpm;
