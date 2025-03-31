use crate::action::KeyAction;
use crate::hid_state::{HidModifiers, HidMouseButtons};
use crate::light::LedIndicator;
use core::ops::{BitAnd, BitOr, Not};

// Max number of fork behaviors
pub(crate) const FORK_MAX_NUM: usize = 16;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct StateBits {
    pub(crate) modifiers: HidModifiers,
    pub(crate) leds: LedIndicator,
    pub(crate) mouse: HidMouseButtons,
    // note: layer active states could be added too if needed
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
    pub const fn new_from(
        modifiers: HidModifiers,
        leds: LedIndicator,
        mouse: HidMouseButtons,
    ) -> Self {
        StateBits {
            modifiers: modifiers,
            leds: leds,
            mouse: mouse,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Fork {
    pub(crate) trigger: KeyAction,
    pub(crate) negative_output: KeyAction,
    pub(crate) positive_output: KeyAction,
    pub(crate) match_any: StateBits,
    pub(crate) match_none: StateBits,
    pub(crate) kept_modifiers: HidModifiers,
    pub(crate) bindable: bool,
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
        kept_modifiers: HidModifiers,
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
            HidModifiers::default(),
            false,
        )
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ActiveFork {
    pub(crate) replacement: KeyAction, // the final replacement decision of the full fork chain
    pub(crate) suppress: HidModifiers, // aggregate the chain's match_any modifiers here
}
