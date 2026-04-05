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
//! - [`combo`] — `Combo`: combo trigger configuration
//! - [`fork`] — `Fork`, `StateBits`: key-override configuration
//! - [`morse`] — `Morse`, `MorsePattern`, `MorseProfile`, `MorseMode`: tap-dance/tap-hold
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
//! - [`protocol::vial`] — Vial/Via protocol types
//! - [`protocol::rmk`] — RMK native protocol ICD (feature-gated: `rmk_protocol`)
//!
//! ### Build-time
//! - [`constants`] — Generated from `keyboard.toml` by `build.rs`

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

#[cfg(test)]
mod tests {
    use super::varint_max_size;

    /// Validate varint_max_size against known postcard varint encoding sizes
    /// and cross-check with actual postcard serialization.
    #[test]
    fn varint_max_size_matches_postcard() {
        // Known varint size boundaries
        assert_eq!(varint_max_size(0), 1);
        assert_eq!(varint_max_size(1), 1);
        assert_eq!(varint_max_size(127), 1);
        assert_eq!(varint_max_size(128), 2);
        assert_eq!(varint_max_size(16383), 2);
        assert_eq!(varint_max_size(16384), 3);

        // Cross-check: serialize actual values with postcard and verify
        // the varint prefix length doesn't exceed our calculation
        let mut buf = [0u8; 16];
        for &n in &[0usize, 1, 127, 128, 255, 256, 16383, 16384, 65535] {
            let bytes = postcard::to_slice(&n, &mut buf).unwrap();
            assert!(
                bytes.len() <= varint_max_size(n),
                "varint_max_size({n}) = {} but postcard used {} bytes",
                varint_max_size(n),
                bytes.len()
            );
        }
    }
}
