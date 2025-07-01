use embassy_time::Duration;

use crate::action::KeyAction;

#[derive(Clone, Debug)]
pub struct TapDance {
    tap: KeyAction,
    hold: KeyAction,
    hold_after_tap: KeyAction,
    double_tap: KeyAction,
    tapping_term: Duration,
}
