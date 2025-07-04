use embassy_time::Duration;

use crate::action::KeyAction;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDance {
    pub tap: KeyAction,
    pub hold: KeyAction,
    pub hold_after_tap: KeyAction,
    pub double_tap: KeyAction,
    pub tapping_term: Duration,
}

impl Default for TapDance {
    fn default() -> Self {
        Self {
            tap: KeyAction::No,
            hold: KeyAction::No,
            hold_after_tap: KeyAction::No,
            double_tap: KeyAction::No,
            tapping_term: Duration::from_millis(200),
        }
    }
}

impl TapDance {
    pub fn new(
        tap: KeyAction,
        hold: KeyAction,
        hold_after_tap: KeyAction,
        double_tap: KeyAction,
        tapping_term: Duration,
    ) -> Self {
        Self {
            tap,
            hold,
            hold_after_tap,
            double_tap,
            tapping_term,
        }
    }
}
