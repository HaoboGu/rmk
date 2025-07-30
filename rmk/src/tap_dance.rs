use embassy_time::Duration;
use heapless::Vec;

use crate::TAP_DANCE_MAX_TAP;
use crate::action::Action;
use crate::morse::{Morse, MorseActions};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDance(pub(crate) Morse<TAP_DANCE_MAX_TAP>);

impl Default for TapDance {
    fn default() -> Self {
        Self(Morse {
            tap_actions: MorseActions::empty(),
            hold_actions: MorseActions::empty(),
            timeout_ms: 200,
            mode: crate::morse::MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        })
    }
}

impl TapDance {
    pub fn new_from_vial(tap: Action, hold: Action, hold_after_tap: Action, double_tap: Action, timeout: u16) -> Self {
        assert!(TAP_DANCE_MAX_TAP >= 2, "TAP_DANCE_MAX_TAP must be at least 2");
        let mut tap_actions = [Action::No; TAP_DANCE_MAX_TAP];
        let mut hold_actions = [Action::No; TAP_DANCE_MAX_TAP];
        tap_actions[0] = tap;
        tap_actions[1] = double_tap;
        hold_actions[0] = hold;
        hold_actions[1] = hold_after_tap;
        Self(Morse::new_tap_dance(tap_actions, hold_actions, timeout))
    }

    /// Create a new tap dance with custom actions for each tap count
    /// This allows for more flexible tap dance configurations
    pub fn new_with_actions(
        tap_actions: Vec<Action, TAP_DANCE_MAX_TAP>,
        hold_actions: Vec<Action, TAP_DANCE_MAX_TAP>,
        timeout: Duration,
    ) -> Self {
        assert!(TAP_DANCE_MAX_TAP >= 2, "TAP_DANCE_MAX_TAP must be at least 2");
        let mut tap_actions_slice = [Action::No; TAP_DANCE_MAX_TAP];
        let mut hold_actions_slice = [Action::No; TAP_DANCE_MAX_TAP];
        for (i, item) in tap_actions.iter().enumerate() {
            tap_actions_slice[i] = *item;
        }
        for (i, item) in hold_actions.iter().enumerate() {
            hold_actions_slice[i] = *item;
        }
        Self(Morse::new_tap_dance(
            tap_actions_slice,
            hold_actions_slice,
            timeout.as_millis() as u16,
        ))
    }

    /// Check if this tap dance has any actions defined
    pub fn has_actions(&self) -> bool {
        !self.0.tap_actions.is_empty() || !self.0.hold_actions.is_empty()
    }
}
