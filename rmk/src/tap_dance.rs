use heapless::Vec;

use crate::TAP_DANCE_MAX_TAP;
use crate::action::Action;
use crate::morse::MorseKeyMode;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDance {
    /// array of (tap, hold) action pairs
    pub(crate) actions: [(Action, Action); TAP_DANCE_MAX_TAP],
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
            actions: [(Action::No, Action::No); TAP_DANCE_MAX_TAP],
            timeout_ms: 200,
            mode: crate::morse::MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }
}

impl TapDance {
    pub fn new_from_vial(tap: Action, hold: Action, hold_after_tap: Action, double_tap: Action, timeout: u16) -> Self {
        assert!(TAP_DANCE_MAX_TAP >= 2, "TAP_DANCE_MAX_TAP must be at least 2");
        let mut actions = [(Action::No, Action::No); TAP_DANCE_MAX_TAP];
        actions[0] = (tap, hold);
        actions[1] = (double_tap, hold_after_tap);
        Self {
            actions: actions,
            timeout_ms: timeout,
            ..Default::default()
        }
    }

    /// Create a new tap dance with custom actions for each tap count
    /// This allows for more flexible tap dance configurations
    pub fn new_with_actions(
        tap_actions: Vec<Action, TAP_DANCE_MAX_TAP>,
        hold_actions: Vec<Action, TAP_DANCE_MAX_TAP>,
        timeout: u16,
    ) -> Self {
        assert!(TAP_DANCE_MAX_TAP >= 2, "TAP_DANCE_MAX_TAP must be at least 2");
        let mut actions = [(Action::No, Action::No); TAP_DANCE_MAX_TAP];
        for (i, item) in tap_actions.iter().enumerate() {
            actions[i].0 = *item;
        }
        for (i, item) in hold_actions.iter().enumerate() {
            actions[i].1 = *item;
        }
        Self {
            actions: actions,
            timeout_ms: timeout,
            ..Default::default()
        }
    }

    pub fn max_pattern_length(&self) -> usize {
        let mut i = TAP_DANCE_MAX_TAP;
        while i > 0 {
            let (tap, hold) = self.actions[i - 1];
            if tap != Action::No || hold != Action::No {
                return i;
            }
            i -= 1;
        }
        i
    }
}
