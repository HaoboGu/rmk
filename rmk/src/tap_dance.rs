use crate::action::Action;
use crate::morse::MorseKeyMode;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDance {
    /// if more complex pattens are needed than these below, use `Morse` instead:
    pub tap_action: Action,
    pub hold_action: Action,
    pub double_tap_action: Action,
    pub hold_after_tap_action: Action,
    /// The timeout time for each operation in milliseconds
    pub timeout_ms: u16,
    /// The decision mode of the morse key
    pub mode: MorseKeyMode,
    /// If the unilateral tap is enabled
    pub unilateral_tap: bool,
}

impl Default for TapDance {
    fn default() -> Self {
        Self {
            tap_action: Action::No,
            hold_action: Action::No,
            double_tap_action: Action::No,
            hold_after_tap_action: Action::No,
            timeout_ms: 200,
            mode: crate::morse::MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }
}

impl TapDance {
    pub fn new_from_vial(tap: Action, hold: Action, hold_after_tap: Action, double_tap: Action, timeout: u16) -> Self {
        Self {
            tap_action: tap,
            hold_action: hold,
            double_tap_action: double_tap,
            hold_after_tap_action: hold_after_tap,
            timeout_ms: timeout,
            mode: crate::morse::MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }

    /// Check if this tap dance has any actions defined
    pub fn has_actions(&self) -> bool {
        self.tap_action != Action::No
            || self.hold_action != Action::No
            || self.double_tap_action != Action::No
            || self.hold_after_tap_action != Action::No
    }

    pub fn max_pattern_length(&self) -> usize {
        if self.double_tap_action != Action::No || self.hold_after_tap_action != Action::No {
            2
        } else {
            1
        }
    }
}
