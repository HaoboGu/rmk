//! # RMK Types
//!
//! This crate provides fundamental type definitions and data structures used in RMK.
//!
//! ## Modules
//!
//! ### Core Modules
//! - [`action`] - Keyboard actions and behaviors (key presses, layer operations, macros)
//! - [`ble`] - BLE profile and connection status types
//! - [`keycode`] - Keycode definitions including HID keycodes, media keys, and system control keycodes
//! - [`fork`] - Shared fork/key-override state types
//! - [`modifier`] - Modifier key combinations and operations
//! - [`mouse_button`] - Mouse button state and combinations
//! - [`led_indicator`] - LED indicator states and operations
//! - [`protocol`] - Communication protocol
//!
//!
//! ## Integration with RMK Ecosystem
//!
//! This crate is designed to work with other RMK components:
//!
//! - **rmk**: Core firmware logic uses these types for state management
//! - **rmk-config**: Configuration parsing produces these types
//! - **rmk-macro**: Code generation macros work with these type definitions

#![no_std]

pub mod action;
pub mod battery;
pub mod ble;
pub mod combo;
pub mod connection;
pub mod constants;
pub mod fork;
pub mod keycode;
pub mod led_indicator;
pub mod modifier;
pub mod morse;
pub mod mouse_button;
pub mod protocol;

/// Compute the maximum varint-encoded length for a given max value.
/// Mirrors `postcard`'s internal `varint_size`.
pub(crate) const fn varint_max_size(max_n: usize) -> usize {
    const BITS_PER_BYTE: usize = 8;
    const BITS_PER_VARINT_BYTE: usize = 7;
    if max_n == 0 {
        return 1;
    }
    let bits = core::mem::size_of::<usize>() * BITS_PER_BYTE - max_n.leading_zeros() as usize;
    let roundup_bits = bits + (BITS_PER_VARINT_BYTE - 1);
    roundup_bits / BITS_PER_VARINT_BYTE
}
