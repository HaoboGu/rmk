//! # RMK Types
//!
//! Shared type definitions used across the RMK keyboard firmware ecosystem.
//!
//! ## Modules
//!
//! ### Actions & keycodes
//! - [`action`] — What keys do: `Action`, `KeyAction`, `EncoderAction`, `LightAction`, etc.
//! - [`keycode`] — What keys are: `KeyCode`, `HidKeyCode`, `ConsumerKey`, `SystemControlKey`
//!
//! ### Behaviors (key overrides, combos, tap-dance)
//! - [`combo`] — `Combo<N>`: combo trigger configuration
//! - [`fork`] — `Fork`, `StateBits`: key-override configuration
//! - [`morse`] — `Morse<N>`, `MorsePattern`, `MorseProfile`, `MorseMode`: tap-dance/tap-hold
//!
//! ### Hardware state
//! - [`modifier`] — `ModifierCombination` bitfield
//! - [`mouse_button`] — `MouseButtons` bitfield
//! - [`led_indicator`] — `LedIndicator` bitfield
//! - [`battery`] — `BatteryStatus`, `ChargeState`
//! - [`ble`] — `BleStatus`, `BleState`
//! - [`connection`] — `ConnectionType` (USB/BLE)
//!
//! ### Protocol
//! - [`protocol::Vec`] — Dual-target Vec (heapless on firmware, alloc on host)
//! - [`protocol::vial`] — Vial/Via protocol types
//! - [`protocol::rmk`] — RMK native protocol ICD (feature-gated: `rmk_protocol`)
//!
//! ### Build-time
//! - [`constants`] — Generated from `keyboard.toml` by `build.rs`

// The postcard-rpc endpoints! macro performs heavy const-eval for type uniqueness checks.
#![allow(long_running_const_eval)]
#![no_std]

#[cfg(feature = "host")]
extern crate alloc;

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
