use heapless::Vec;

use crate::MAX_PATTERNS_PER_KEY;
use crate::action::Action;

/// a sequence of maximum 15 tap or hold can be encoded on an u16:
/// 0x1 when empty, then 0 for tap or 1 for hold shifted from the right
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorsePattern(u16);

pub const TAP: MorsePattern = MorsePattern(0b10);
pub const HOLD: MorsePattern = MorsePattern(0b11);
pub const DOUBLE_TAP: MorsePattern = MorsePattern(0b100);
pub const HOLD_AFTER_TAP: MorsePattern = MorsePattern(0b101);

impl MorsePattern {
    pub fn max_taps() -> usize {
        15 // 15 taps can be encoded on u16 bits (1 bit used to mark the start position)
    }

    pub fn default() -> Self {
        MorsePattern(0b1) // 0b1 means empty
    }

    pub fn from_u16(value: u16) -> Self {
        MorsePattern(value)
    }

    pub fn to_u16(&self) -> u16 {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0b1
    }

    pub fn is_full(&self) -> bool {
        (self.0 & 0b1000_0000_0000_0000) != 0
    }

    pub fn pattern_length(&self) -> usize {
        15 - self.0.leading_zeros() as usize
    }

    /// checks if this pattern starts with the given one
    pub fn starts_with(&self, pattern_start: MorsePattern) -> bool {
        let n = pattern_start.0.leading_zeros();
        let m = self.0.leading_zeros();
        m <= n && (self.0 >> (n - m) == pattern_start.0)
    }

    pub fn last_is_hold(&self) -> bool {
        self.0 & 0b1 == 0b1
    }

    pub fn followed_by_tap(&self) -> Self {
        // Shift the bits to the left and set the last bit to 0 (tap)
        MorsePattern((self.0 << 1) | 0b0)
    }

    pub fn followed_by_hold(&self) -> Self {
        // Shift the bits to the left and set the last bit to 1 (hold)
        MorsePattern((self.0 << 1) | 0b1)
    }
}

/// Definition of a tap dance key.
///
/// A tap dance key is a key that behaves differently according to the pattern of a tap/hold sequence.
/// The maximum number of taps is limited to 15 by the internal u16 representation of MorsePattern.
/// There is a lists of (pattern, corresponding action) pairs for each tap_dance key:
/// The number of pairs is limited by MAX_PATTERNS_PER_KEY, which is a const generic parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDance {
    /// The timeout time for each operation in milliseconds
    pub timeout_ms: u16,
    /// The decision mode of the morse key
    pub mode: TapHoldMode,
    /// If the unilateral tap is enabled
    pub unilateral_tap: bool,
    /// The list of pattern -> action pairs, which can be triggered
    pub actions: Vec<(MorsePattern, Action), MAX_PATTERNS_PER_KEY>,
}

impl Default for TapDance {
    fn default() -> Self {
        Self {
            timeout_ms: 250,
            mode: TapHoldMode::HoldOnOtherPress,
            unilateral_tap: false,
            actions: Vec::default(),
        }
    }
}

impl TapDance {
    pub fn new_from_vial(tap: Action, hold: Action, hold_after_tap: Action, double_tap: Action, timeout: u16) -> Self {
        let mut result = Self::default();
        if tap != Action::No {
            _ = result.actions.push((TAP, tap));
        }
        if hold != Action::No {
            _ = result.actions.push((HOLD, hold));
        }
        if double_tap != Action::No {
            _ = result.actions.push((DOUBLE_TAP, double_tap));
        }
        if hold_after_tap != Action::No {
            _ = result.actions.push((HOLD_AFTER_TAP, hold_after_tap));
        }
        result.timeout_ms = timeout;
        result
    }

    /// Create a new tap dance with custom actions for each tap count
    /// This allows for more flexible tap dance configurations
    pub fn new_with_actions(
        tap_actions: Vec<Action, MAX_PATTERNS_PER_KEY>,
        hold_actions: Vec<Action, MAX_PATTERNS_PER_KEY>,
        timeout: u16,
    ) -> Self {
        assert!(MAX_PATTERNS_PER_KEY >= 4, "MAX_PATTERNS_PER_KEY must be at least 4");
        let mut result = Self::default();
        result.timeout_ms = timeout;

        let mut pattern = 0b1u16;
        for item in tap_actions.iter() {
            pattern = pattern << 1;
            _ = result.put(MorsePattern::from_u16(pattern), *item); //+ one tap in each iteration
        }

        let mut pattern = 0b1u16;
        for item in hold_actions.iter() {
            pattern = pattern << 1;
            _ = result.put(MorsePattern::from_u16(pattern | 0b1), *item); //+ one tap in each iteration, but the last one is modified to hold
        }

        result
    }

    /// checks all stored pattern if more than one continuation found for the given pattern, none, otherwise the unique completion
    pub fn try_predict_final(&self, pattern_start: MorsePattern) -> Option<Action> {
        let mut first = None;
        for pair in self.actions.iter() {
            //if pair.pattern starts with the given pattern_start
            if pair.0.starts_with(pattern_start) {
                if first.is_none() {
                    first = Some(pair);
                } else {
                    return None;
                }
            }
        }
        first.map(|first| first.1)
    }

    pub fn get(&self, pattern: MorsePattern) -> Option<Action> {
        for pair in self.actions.iter() {
            if pair.0 == pattern {
                return Some(pair.1);
            }
        }
        None
    }

    /// A call with Action::No will remove the item from the collection,
    /// otherwise will update the existing action or insert the new action if possible
    pub fn put(&mut self, pattern: MorsePattern, action: Action) -> Result<(), Action> {
        if action != Action::No {
            for pair in self.actions.iter_mut() {
                if pair.0 == pattern {
                    pair.1 = action; //modify
                    return Ok(());
                }
            }
            self.actions.push((pattern, action)).map_err(|v| v.1) //try to insert
        } else {
            //try to remove
            for i in 0..self.actions.len() {
                if self.actions[i].0 == pattern {
                    self.actions[i] = self.actions[self.actions.len() - 1];
                    self.actions.pop();
                    return Ok(());
                }
            }
            Ok(())
        }
    }
}

/// Mode for morse key behavior
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TapHoldMode {
    /// Normal mode, the decision is made when timeout
    Normal,
    /// Same as QMK's permissive hold: https://docs.qmk.fm/tap_hold#tap-or-hold-decision-modes
    /// When another key is pressed and released during the current morse key is held,
    /// the hold action of current morse key will be triggered
    PermissiveHold,
    /// Trigger hold immediately if any other non-morse key is pressed when the current morse key is held
    HoldOnOtherPress,
}
