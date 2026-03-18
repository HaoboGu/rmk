//! Shared fork-related types used by firmware and protocol layers.

use core::ops::{BitAnd, BitOr, Not};

use crate::led_indicator::LedIndicator;
use crate::modifier::ModifierCombination;
use crate::mouse_button::MouseButtons;

/// Bitset state used by fork matching logic.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    postcard::experimental::max_size::MaxSize,
    postcard_schema::Schema,
)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct StateBits {
    pub modifiers: ModifierCombination,
    pub leds: LedIndicator,
    pub mouse: MouseButtons,
}

impl BitOr for StateBits {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            modifiers: self.modifiers | rhs.modifiers,
            leds: self.leds | rhs.leds,
            mouse: self.mouse | rhs.mouse,
        }
    }
}

impl BitAnd for StateBits {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self {
            modifiers: self.modifiers & rhs.modifiers,
            leds: self.leds & rhs.leds,
            mouse: self.mouse & rhs.mouse,
        }
    }
}

impl Not for StateBits {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self {
            modifiers: !self.modifiers,
            leds: !self.leds,
            mouse: !self.mouse,
        }
    }
}

impl StateBits {
    pub const fn new_from(modifiers: ModifierCombination, leds: LedIndicator, mouse: MouseButtons) -> Self {
        Self { modifiers, leds, mouse }
    }
}
