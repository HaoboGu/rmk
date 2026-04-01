//! Shared fork-related types used by firmware and protocol layers.

use core::ops::{BitAnd, BitOr, Not};

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::action::KeyAction;
use crate::led_indicator::LedIndicator;
use crate::modifier::ModifierCombination;
use crate::mouse_button::MouseButtons;

/// Bitset state used by fork matching logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
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

/// Fork (key override) configuration.
///
/// A fork overrides a key's output based on the current modifier/LED/mouse state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Fork {
    pub trigger: KeyAction,
    pub negative_output: KeyAction,
    pub positive_output: KeyAction,
    pub match_any: StateBits,
    pub match_none: StateBits,
    pub kept_modifiers: ModifierCombination,
    pub bindable: bool,
}

impl Default for Fork {
    fn default() -> Self {
        Self::empty()
    }
}

impl Fork {
    pub fn new(
        trigger: KeyAction,
        negative_output: KeyAction,
        positive_output: KeyAction,
        match_any: StateBits,
        match_none: StateBits,
        kept_modifiers: ModifierCombination,
        bindable: bool,
    ) -> Self {
        Self {
            trigger,
            negative_output,
            positive_output,
            match_any,
            match_none,
            kept_modifiers,
            bindable,
        }
    }

    pub fn new_ex(
        trigger: KeyAction,
        negative_output: KeyAction,
        positive_output: KeyAction,
        match_any: StateBits,
        match_none: StateBits,
        kept: StateBits,
        bindable: bool,
    ) -> Self {
        Self {
            trigger,
            negative_output,
            positive_output,
            match_any,
            match_none,
            kept_modifiers: kept.modifiers,
            bindable,
        }
    }

    pub fn empty() -> Self {
        Self::new(
            KeyAction::No,
            KeyAction::No,
            KeyAction::No,
            StateBits::default(),
            StateBits::default(),
            ModifierCombination::default(),
            false,
        )
    }
}
