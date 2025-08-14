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

    pub fn is_empty(&self) -> bool {
        self.0 == 0b1
    }

    pub fn is_full(&self) -> bool {
        (self.0 & 0b1000_0000_0000_0000) != 0
    }

    pub fn pattern_length(&self) -> usize {
        15 - self.0.leading_zeros() as usize
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

/// Definition of a morse key.
///
/// A morse key is a key that behaves differently according to the pattern and number of taps and holds.
///
/// There is a lists of (morse pattern, actions) pairs for each morse key:
/// The number of pairs is limited by N, which is a const generic parameter.
///
/// The maximum number of taps is limited to 15 by the internal u16 representation of MorsePattern.
///
/// The morse key can act as a superset of tap-hold key and tap-dance key and more if N >= 4.

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Morse<const N: usize> {
    /// The list of pattern -> actions pairs, which can be triggered
    //pub(crate) actions: MorseActions<N>,
    pub(crate) actions: heapless::Vec<(MorsePattern, Action), N>,

    /// The timeout time for each operation in milliseconds
    pub timeout_ms: u16,
    /// The decision mode of the morse key
    pub mode: MorseKeyMode,
    /// If the unilateral tap is enabled
    pub unilateral_tap: bool,
}

impl<const N: usize> Default for Morse<N> {
    fn default() -> Self {
        Self {
            actions: heapless::Vec::default(),
            timeout_ms: 250,
            mode: MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }
}

impl<const N: usize> Morse<N> {
    pub fn max_pattern_length(&self) -> usize {
        let mut max_length = 0;
        for pair in self.actions.iter() {
            let pattern_length = pair.0.pattern_length();
            if pattern_length > max_length {
                max_length = pattern_length;
            }
        }
        max_length
    }

    pub fn get(&self, pattern: MorsePattern) -> Option<&Action> {
        for pair in self.actions.iter() {
            if pair.0 == pattern {
                return Some(&pair.1);
            }
        }
        None
    }
}

/// Mode for morse key behavior
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MorseKeyMode {
    /// Normal mode, the decision is made when timeout
    Normal,
    /// Same as QMK's permissive hold: https://docs.qmk.fm/tap_hold#tap-or-hold-decision-modes
    /// When another key is pressed and released during the current morse key is held,
    /// the hold action of current morse key will be triggered
    PermissiveHold,
    /// Trigger hold immediately if any other non-morse key is pressed when the current morse key is held
    HoldOnOtherPress,
}
