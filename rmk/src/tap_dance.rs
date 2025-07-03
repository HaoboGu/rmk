use embassy_time::Duration;

use crate::action::KeyAction;

#[derive(Clone, Debug)]
pub struct TapDance {
    pub tap: KeyAction,
    pub hold: KeyAction,
    pub hold_after_tap: KeyAction,
    pub double_tap: KeyAction,
    pub tapping_term: Duration,
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
