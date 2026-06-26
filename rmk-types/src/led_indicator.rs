//! LED indicator.
//!
//! This module handles keyboard LED indicators such as Caps Lock, Num Lock,
//! and Scroll Lock. It provides efficient bitfield operations for these indicators.
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use bitfield_struct::bitfield;
use postcard::experimental::max_size::MaxSize;

/// Indicators defined in the HID spec 11.1
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LedIndicatorType {
    NumLock,
    CapsLock,
    ScrollLock,
    Compose,
    Kana,
}

#[bitfield(u8, defmt = cfg(feature = "defmt"))]
#[derive(Eq, PartialEq, MaxSize)]
pub struct LedIndicator {
    #[bits(1)]
    pub num_lock: bool,
    #[bits(1)]
    pub caps_lock: bool,
    #[bits(1)]
    pub scroll_lock: bool,
    #[bits(1)]
    pub compose: bool,
    #[bits(1)]
    pub kana: bool,
    #[bits(3)]
    _reserved: u8,
}

// `u8` on postcard (wire unchanged), `{ num_lock, … }` named-bools on JSON/wasm.
crate::flag_bitfield_serde!(LedIndicator, LedIndicatorFlags, {
    num_lock = with_num_lock,
    caps_lock = with_caps_lock,
    scroll_lock = with_scroll_lock,
    compose = with_compose,
    kana = with_kana,
});

impl BitOr for LedIndicator {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}

impl BitAnd for LedIndicator {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() & rhs.into_bits())
    }
}

impl Not for LedIndicator {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_bits(!self.into_bits())
    }
}

impl BitAndAssign for LedIndicator {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl BitOrAssign for LedIndicator {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl LedIndicator {
    pub const NUM_LOCK: Self = Self::new().with_num_lock(true);
    pub const CAPS_LOCK: Self = Self::new().with_caps_lock(true);
    pub const SCROLL_LOCK: Self = Self::new().with_scroll_lock(true);
    pub const COMPOSE: Self = Self::new().with_compose(true);
    pub const KANA: Self = Self::new().with_kana(true);

    pub const fn new_from(num_lock: bool, caps_lock: bool, scroll_lock: bool, compose: bool, kana: bool) -> Self {
        Self::new()
            .with_num_lock(num_lock)
            .with_caps_lock(caps_lock)
            .with_scroll_lock(scroll_lock)
            .with_compose(compose)
            .with_kana(kana)
    }
}

/// `flag_bitfield_serde!` parity, exercised for all three bitfields: the
/// postcard wire stays a compact `u8` (byte-identical to the old derived serde —
/// the `wire_*.snap` snapshots pin the same), while a human-readable format
/// (serde_json, mirroring serde-wasm-bindgen) emits a struct of named `bool`s
/// and round-trips back to the same value.
#[cfg(test)]
mod serde_repr_tests {
    use crate::led_indicator::LedIndicator;
    use crate::modifier::ModifierCombination;
    use crate::mouse_button::MouseButtons;

    #[test]
    fn postcard_stays_one_byte() {
        let mut buf = [0u8; 4];
        let led = LedIndicator::NUM_LOCK | LedIndicator::SCROLL_LOCK;
        assert_eq!(postcard::to_slice(&led, &mut buf).unwrap(), &[led.into_bits()]);
        let m = ModifierCombination::LCTRL | ModifierCombination::RGUI;
        assert_eq!(postcard::to_slice(&m, &mut buf).unwrap(), &[m.into_bits()]);
        let b = MouseButtons::BUTTON1 | MouseButtons::BUTTON8;
        assert_eq!(postcard::to_slice(&b, &mut buf).unwrap(), &[b.into_bits()]);
    }

    #[test]
    fn json_is_named_bools_and_round_trips() {
        let led = LedIndicator::CAPS_LOCK | LedIndicator::KANA;
        let j = serde_json::to_value(led).unwrap();
        assert_eq!(j["caps_lock"], serde_json::json!(true));
        assert_eq!(j["kana"], serde_json::json!(true));
        assert_eq!(j["num_lock"], serde_json::json!(false));
        assert!(j.get("scroll_lock").is_some(), "every flag is a named bool");
        assert_eq!(serde_json::from_value::<LedIndicator>(j).unwrap(), led);

        let m = ModifierCombination::LSHIFT | ModifierCombination::RALT;
        let mj = serde_json::to_value(m).unwrap();
        assert_eq!(mj["left_shift"], serde_json::json!(true));
        assert_eq!(mj["right_alt"], serde_json::json!(true));
        assert_eq!(serde_json::from_value::<ModifierCombination>(mj).unwrap(), m);

        let b = MouseButtons::BUTTON2 | MouseButtons::BUTTON3;
        let bj = serde_json::to_value(b).unwrap();
        assert_eq!(bj["button2"], serde_json::json!(true));
        assert_eq!(bj["button3"], serde_json::json!(true));
        assert_eq!(serde_json::from_value::<MouseButtons>(bj).unwrap(), b);
    }
}
