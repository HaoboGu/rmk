use heapless::{LinearMap, Vec};
use rmk_types::action::{Action, MorseProfile};

use crate::MAX_PATTERNS_PER_KEY;

/// MorsePattern is a sequence of maximum 15 taps or holds that can be encoded into an u16:
/// 0x1 when empty, then 0 for tap or 1 for hold shifted from the right
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorsePattern(u16);

pub const TAP: MorsePattern = MorsePattern(0b10);
pub const HOLD: MorsePattern = MorsePattern(0b11);
pub const DOUBLE_TAP: MorsePattern = MorsePattern(0b100);
pub const HOLD_AFTER_TAP: MorsePattern = MorsePattern(0b101);

impl Default for MorsePattern {
    fn default() -> Self {
        MorsePattern(0b1) // 0b1 means empty
    }
}

impl MorsePattern {
    pub fn max_taps() -> usize {
        15 // 15 taps can be encoded on u16 bits (1 bit used to mark the start position)
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

    /// Checks if this pattern starts with the given one
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
        MorsePattern(self.0 << 1)
    }

    pub fn followed_by_hold(&self) -> Self {
        // Shift the bits to the left and set the last bit to 1 (hold)
        MorsePattern((self.0 << 1) | 0b1)
    }
}

/// Definition of a morse key.
///
/// A morse key is a key that behaves differently according to the pattern of a tap/hold sequence.
/// The maximum number of taps is limited to 15 by the internal u16 representation of MorsePattern.
/// There is a lists of (pattern, corresponding action) pairs for each morse key:
/// The number of pairs is limited by MAX_PATTERNS_PER_KEY, which is a const generic parameter.
#[derive(Debug, Clone)]
// #[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Morse<const NUM_PATTERNS: usize = MAX_PATTERNS_PER_KEY> {
    /// The profile of this morse key, which defines the timing parameters, etc.
    /// If some of its fields are filled with None, the global default value will be used.
    pub profile: MorseProfile,
    /// The list of pattern -> action pairs, which can be triggered
    pub actions: LinearMap<MorsePattern, Action, NUM_PATTERNS>,
}

#[cfg(feature = "defmt")]
impl<const NUM_PATTERNS: usize> defmt::Format for Morse<NUM_PATTERNS> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "profile: MorseProfile({:?}), ", self.profile);
        defmt::write!(f, "actions: [");
        for item in self.actions.iter() {
            defmt::write!(f, "{:?},", item);
        }
        defmt::write!(f, "]");
    }
}

impl Default for Morse {
    fn default() -> Self {
        Self {
            profile: MorseProfile::const_default(),
            actions: LinearMap::default(),
        }
    }
}

impl Morse {
    pub fn new_from_vial(
        tap: Action,
        hold: Action,
        hold_after_tap: Action,
        double_tap: Action,
        profile: MorseProfile,
    ) -> Self {
        let mut result = Self {
            profile,
            ..Default::default()
        };

        if tap != Action::No {
            _ = result.actions.insert(TAP, tap);
        }
        if hold != Action::No {
            _ = result.actions.insert(HOLD, hold);
        }
        if double_tap != Action::No {
            _ = result.actions.insert(DOUBLE_TAP, double_tap);
        }
        if hold_after_tap != Action::No {
            _ = result.actions.insert(HOLD_AFTER_TAP, hold_after_tap);
        }
        result
    }

    /// Create a new morse with custom actions for each tap count
    /// This allows for more flexible morse configurations
    pub fn new_with_actions(
        tap_actions: Vec<Action, MAX_PATTERNS_PER_KEY>,
        hold_actions: Vec<Action, MAX_PATTERNS_PER_KEY>,
        profile: MorseProfile,
    ) -> Self {
        assert!(MAX_PATTERNS_PER_KEY >= 4, "MAX_PATTERNS_PER_KEY must be at least 4");
        let mut result = Self {
            profile,
            ..Default::default()
        };

        let mut pattern = 0b1u16;
        for item in tap_actions.iter() {
            pattern <<= 1;
            result.put(MorsePattern::from_u16(pattern), *item); //+ one tap in each iteration
        }

        let mut pattern = 0b1u16;
        for item in hold_actions.iter() {
            pattern <<= 1;
            result.put(MorsePattern::from_u16(pattern | 0b1), *item); //+ one tap in each iteration, but the last one is modified to hold
        }

        result
    }

    pub fn max_pattern_length(&self) -> usize {
        let mut max_length = 0;
        for pair in self.actions.iter() {
            max_length = max_length.max(pair.0.pattern_length());
        }
        max_length
    }

    /// Checks all stored patterns if more than one continuation found for the given pattern, None,
    /// otherwise the unique completion
    pub fn try_predict_final_action(&self, pattern_start: MorsePattern) -> Option<Action> {
        // Check whether current pattern matches any of the legal patterns.
        // If not, return early
        if !self.actions.contains_key(&pattern_start) {
            return None;
        }

        let mut first: Option<&Action> = None;
        for pair in self.actions.iter() {
            // If pair.pattern starts with the given pattern_start
            if pair.0.starts_with(pattern_start) {
                if let Some(action) = first {
                    if *action != *pair.1 {
                        return None; //the solution is not unique, so must wait for possible continuation
                    }
                } else {
                    first = Some(pair.1);
                }
            }
        }

        first.copied()
    }

    pub fn get(&self, pattern: MorsePattern) -> Option<Action> {
        self.actions.get(&pattern).copied()
    }

    /// A call with Action::No will remove the item from the collection,
    /// otherwise will update the existing action or insert the new action if possible
    pub fn put(&mut self, pattern: MorsePattern, action: Action) {
        if action != Action::No {
            if let Err(a) = self.actions.insert(pattern, action) {
                error!("The actions buffer is full in current morse key, pushing {:?} fails", a);
            }
        } else {
            let _ = self.actions.remove(&pattern);
        }
    }
}
