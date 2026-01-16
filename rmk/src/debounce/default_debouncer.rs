use core::num::NonZeroU16;

use embassy_time::Instant;

use super::{DebounceState, DebouncerTrait};
use crate::DEBOUNCE_THRESHOLD;
use crate::matrix::KeyState;

/// Tracks the debounce state of a single key.
#[derive(Copy, Clone, Debug, PartialEq)]
enum DebounceCounter {
    /// The key is in a stable state (idle).
    Idle,
    /// The key is in a transient state (debouncing).
    /// The payload represents the **start timestamp** of the state change.
    ///
    /// optimization: `NonZeroU16` allows Rust to apply Null Pointer Optimization (NPO),
    /// making `DebounceCounter` occupy only 2 bytes in memory (0 = Idle).
    Debouncing(NonZeroU16),
}

pub struct DefaultDebouncer<const ROW: usize, const COL: usize> {
    counters: [[DebounceCounter; ROW]; COL],
}

impl<const ROW: usize, const COL: usize> Default for DefaultDebouncer<ROW, COL> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const ROW: usize, const COL: usize> DefaultDebouncer<ROW, COL> {
    pub fn new() -> Self {
        DefaultDebouncer {
            counters: [[DebounceCounter::Idle; ROW]; COL],
        }
    }
}

impl<const ROW: usize, const COL: usize> DebouncerTrait<ROW, COL> for DefaultDebouncer<ROW, COL> {
    fn detect_change_with_debounce(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        key_active: bool,
        key_state: &KeyState,
    ) -> DebounceState {
        let counter = &mut self.counters[col_idx][row_idx];

        // If the current physical key_active state matches the registered key_state,
        // the key is stable, no debouncing is needed.
        if key_state.pressed == key_active {
            *counter = DebounceCounter::Idle;
            return DebounceState::Ignored;
        }

        // Handle state change (Debouncing logic).

        // Get the current timestamp.
        // We use `NonZeroU16` to ensure the timestamp is never 0, preserving the NPO optimization.
        // If the timer wraps to 0 (a rare 1ms window every ~65s), we skip this tick.
        let Some(now) = NonZeroU16::new(Instant::now().as_millis() as u16) else {
            return DebounceState::InProgress;
        };

        match counter {
            DebounceCounter::Idle => {
                // Detected a new potential state change.
                // Record the start time and enter the debouncing state.
                *counter = DebounceCounter::Debouncing(now);
                DebounceState::InProgress
            }
            DebounceCounter::Debouncing(start_time) => {
                // Calculate elapsed time, then check the debouncing state
                let elapsed = now.get().wrapping_sub(start_time.get());

                if elapsed >= DEBOUNCE_THRESHOLD {
                    *counter = DebounceCounter::Idle;
                    DebounceState::Debounced
                } else {
                    DebounceState::InProgress
                }
            }
        }
    }
}
