use crate::action::Action;
use crate::keycode::ModifierCombination;

/// a sequence of maximum 15 tap or hold can be encoded on an u16:
/// 0x1 when empty, then 0 for tap or 1 for hold shifted from the right
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorsePattern(u16);

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

    //common patterns:
    pub fn tap() -> Self {
        MorsePattern(0b10) // 0b10 means tap
    }
    pub fn hold() -> Self {
        MorsePattern(0b11) // 0b11 means hold
    }
    pub fn double_tap() -> Self {
        MorsePattern(0b100) // 0b100 means double tap
    }
    pub fn hold_after_tap() -> Self {
        MorsePattern(0b101) // 0b101 means hold after tap
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

    pub fn new_tap_hold(tap_action: Action, hold_action: Action) -> Self {
        Self {
            actions: Self::new_tap_hold_combo(tap_action, hold_action),
            timeout_ms: 250,
            mode: MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }

    pub fn new_layer_tap_hold(tap_action: Action, layer: u8) -> Self {
        Self {
            actions: Self::new_tap_hold_combo(tap_action, Action::LayerOn(layer)),
            timeout_ms: 250,
            mode: MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }

    pub fn new_modifier_tap_hold(tap_action: Action, modifier: ModifierCombination) -> Self {
        Self {
            actions: Self::new_tap_hold_combo(tap_action, Action::Modifier(modifier)),
            timeout_ms: 250,
            mode: MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }

    pub const fn new_hrm(tap_action: Action, modifier: ModifierCombination, timeout_ms: u16) -> Self {
        Self {
            actions: Self::new_tap_hold_combo(tap_action, Action::Modifier(modifier)),
            timeout_ms,
            mode: MorseKeyMode::PermissiveHold,
            unilateral_tap: true,
        }
    }

    pub fn new_tap_dance(
        tap_action: Action,
        hold_action: Action,
        double_tap_action: Action,
        hold_after_tap_action: Action,
        timeout_ms: u16,
        mode: MorseKeyMode,
        unilateral_tap: bool,
    ) -> Self {
        let mut result = Self {
            actions: Self::new_tap_hold_combo(tap_action, hold_action),
            timeout_ms,
            mode,
            unilateral_tap,
        };
        if double_tap_action != Action::No {
            _ = result.actions.push((MorsePattern::double_tap(), double_tap_action));
        }
        if hold_after_tap_action != Action::No {
            _ = result
                .actions
                .push((MorsePattern::hold_after_tap(), hold_after_tap_action));
        }
        result
    }

    pub fn new_tap_hold_with_config(
        tap_action: Action,
        hold_action: Action,
        timeout_ms: u16,
        mode: MorseKeyMode,
        unilateral_tap: bool,
    ) -> Self {
        Self {
            actions: Self::new_tap_hold_combo(tap_action, hold_action),
            timeout_ms,
            mode,
            unilateral_tap,
        }
    }

    // TODO: Remove the global setting
    pub fn get_timeout(&self, global_timeout_time: u16) -> u16 {
        if self.timeout_ms == 250 && global_timeout_time != 250 {
            // Global setting overrides the default setting
            global_timeout_time
        } else {
            self.timeout_ms
        }
    }

    pub fn action_from_pattern(&self, pattern: MorsePattern) -> Action {
        *self.get(pattern).unwrap_or(&Action::No)
    }

    pub fn tap_action(&self) -> Action {
        *self.get(MorsePattern::tap()).unwrap_or(&Action::No)
    }

    fn new_tap_hold_combo(tap_action: Action, hold_action: Action) -> heapless::Vec<(MorsePattern, Action), N> {
        let mut result = heapless::Vec::<(MorsePattern, Action), N>::new();
        if tap_action != Action::No {
            _ = result.push((MorsePattern::tap(), tap_action));
        }
        if hold_action != Action::No {
            _ = result.push((MorsePattern::hold(), hold_action));
        }
        result
    }

    fn get(&self, pattern: MorsePattern) -> Option<&Action> {
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
