use embassy_time::Instant;

use super::{DebounceState, DebouncerTrait};
use crate::matrix::KeyState;
use crate::DEBOUNCE_THRESHOLD;

/// Fast per-key debouncer.
/// The debouncing algorithm is similar as QMK's [asym eager defer pk debouncer](https://github.com/qmk/qmk_firmware/blob/2fd56317763e8b3b73f0db7488ef42a70f5b946e/quantum/debounce/asym_eager_defer_pk.c)
pub struct RapidDebouncer<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> {
    last_ms: Instant,
    debouncing: [[bool; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> Default
    for RapidDebouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> RapidDebouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM> {
    /// Create a rapid debouncer
    pub fn new() -> Self {
        RapidDebouncer {
            debouncing: [[false; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            last_ms: Instant::now(),
        }
    }
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> DebouncerTrait
    for RapidDebouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Per-key fast debounce
    fn detect_change_with_debounce(
        &mut self,
        in_idx: usize,
        out_idx: usize,
        pin_state: bool,
        key_state: &KeyState,
    ) -> DebounceState {
        let debouncing = self.debouncing[out_idx][in_idx];
        if debouncing {
            // Current key is in debouncing state
            if self.last_ms.elapsed().as_millis() as u16 > DEBOUNCE_THRESHOLD {
                // If the elapsed time > DEBOUNCE_THRESHOLD, reset
                self.debouncing[out_idx][in_idx] = false;
                DebounceState::Ignored
            } else {
                // Still in a debouncing progress
                DebounceState::InProgress
            }
        } else if key_state.pressed != pin_state {
            // If current key isn't in debouncing state, and a key change is detected
            // Trigger the key immediately and record current tick
            self.last_ms = Instant::now();
            // Change debouncing state
            self.debouncing[out_idx][in_idx] = true;
            DebounceState::Debounced
        } else {
            DebounceState::Ignored
        }
    }
}
