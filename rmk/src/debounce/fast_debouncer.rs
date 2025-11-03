use embassy_time::Instant;

use super::{DebounceState, DebouncerTrait};
use crate::DEBOUNCE_THRESHOLD;
use crate::matrix::KeyState;

/// Fast per-key debouncer.
pub struct FastDebouncer<const ROW: usize, const COL: usize> {
    last_ms: Instant,
    debouncing: [[bool; ROW]; COL],
}

impl<const ROW: usize, const COL: usize> Default for FastDebouncer<ROW, COL> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const ROW: usize, const COL: usize> FastDebouncer<ROW, COL> {
    /// Create a fast debouncer
    pub fn new() -> Self {
        FastDebouncer {
            debouncing: [[false; ROW]; COL],
            last_ms: Instant::now(),
        }
    }
}

impl<const ROW: usize, const COL: usize> DebouncerTrait<ROW, COL> for FastDebouncer<ROW, COL> {
    /// Per-key fast debounce
    fn detect_change_with_debounce(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        pin_state: bool,
        key_state: &KeyState,
    ) -> DebounceState {
        let debouncing = self.debouncing[col_idx][row_idx];
        if debouncing {
            // Current key is in debouncing state
            if self.last_ms.elapsed().as_millis() as u16 > DEBOUNCE_THRESHOLD {
                // If the elapsed time > DEBOUNCE_THRESHOLD, reset
                self.debouncing[col_idx][row_idx] = false;
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
            self.debouncing[col_idx][row_idx] = true;
            DebounceState::Debounced
        } else {
            DebounceState::Ignored
        }
    }
}
