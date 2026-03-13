//! Shared fork-related types used by firmware and protocol layers.

use core::ops::{BitAnd, BitOr, Not};

use crate::action::KeyAction;
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
)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ForkStateBits {
    pub modifiers: ModifierCombination,
    pub leds: LedIndicator,
    pub mouse: MouseButtons,
}

impl BitOr for ForkStateBits {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            modifiers: self.modifiers | rhs.modifiers,
            leds: self.leds | rhs.leds,
            mouse: self.mouse | rhs.mouse,
        }
    }
}

impl BitAnd for ForkStateBits {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self {
            modifiers: self.modifiers & rhs.modifiers,
            leds: self.leds & rhs.leds,
            mouse: self.mouse & rhs.mouse,
        }
    }
}

impl Not for ForkStateBits {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self {
            modifiers: !self.modifiers,
            leds: !self.leds,
            mouse: !self.mouse,
        }
    }
}

impl ForkStateBits {
    pub const fn new_from(modifiers: ModifierCombination, leds: LedIndicator, mouse: MouseButtons) -> Self {
        Self { modifiers, leds, mouse }
    }
}

/// A fork (key override) definition.
///
/// When the trigger key is pressed, the fork examines current modifier/LED/mouse
/// state and outputs either `positive_output` (conditions met) or
/// `negative_output` (conditions not met).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    postcard::experimental::max_size::MaxSize,
)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Fork {
    pub trigger: KeyAction,
    pub negative_output: KeyAction,
    pub positive_output: KeyAction,
    pub match_any: ForkStateBits,
    pub match_none: ForkStateBits,
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
        match_any: ForkStateBits,
        match_none: ForkStateBits,
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
        match_any: ForkStateBits,
        match_none: ForkStateBits,
        kept: ForkStateBits,
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
            ForkStateBits::default(),
            ForkStateBits::default(),
            ModifierCombination::default(),
            false,
        )
    }
}
