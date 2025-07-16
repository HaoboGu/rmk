use embassy_time::{Duration, Instant};
use heapless::Vec;

use crate::action::KeyAction;
use crate::TAP_DANCE_MAX_TAP;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDanceState {
    pub tap_count: u8,
    pub last_tap_time: Option<Instant>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDance {
    pub tap_actions: Vec<KeyAction, TAP_DANCE_MAX_TAP>,
    pub hold_actions: Vec<KeyAction, TAP_DANCE_MAX_TAP>,
    pub tapping_term: Duration,
}

impl Default for TapDance {
    fn default() -> Self {
        Self {
            tap_actions: Vec::new(),
            hold_actions: Vec::new(),
            tapping_term: Duration::from_millis(200),
        }
    }
}

impl TapDance {
    pub fn new_from_vial(
        tap: KeyAction,
        hold: KeyAction,
        hold_after_tap: KeyAction,
        double_tap: KeyAction,
        tapping_term: Duration,
    ) -> Self {
        assert!(TAP_DANCE_MAX_TAP >= 2, "TAP_DANCE_MAX_TAP must be at least 2");
        let mut tap_actions = Vec::new();
        let mut hold_actions = Vec::new();
        tap_actions.push(tap).ok();
        hold_actions.push(hold).ok();
        hold_actions.push(hold_after_tap).ok();
        tap_actions.push(double_tap).ok();
        Self {
            tap_actions,
            hold_actions,
            tapping_term,
        }
    }

    /// Create a new tap dance with custom actions for each tap count
    /// This allows for more flexible tap dance configurations
    pub fn new_with_actions(
        tap_actions: Vec<KeyAction, TAP_DANCE_MAX_TAP>,
        hold_actions: Vec<KeyAction, TAP_DANCE_MAX_TAP>,
        tapping_term: Duration,
    ) -> Self {
        Self {
            tap_actions,
            hold_actions,
            tapping_term,
        }
    }

    /// Check if this tap dance has any actions defined
    pub fn has_actions(&self) -> bool {
        !self.tap_actions.is_empty() || !self.hold_actions.is_empty()
    }
}
