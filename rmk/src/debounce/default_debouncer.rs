use embassy_time::Instant;

use super::{DebounceState, DebouncerTrait};
use crate::matrix::KeyState;
use crate::DEBOUNCE_THRESHOLD;

/// Debounce counter info for each key.
#[derive(Copy, Clone, Debug)]
struct DebounceCounter(u16);

impl DebounceCounter {
    fn increase(&mut self, elapsed_ms: u16) {
        // Prevent overflow
        if u16::MAX - self.0 <= elapsed_ms {
            self.0 = u16::MAX;
        } else {
            self.0 += elapsed_ms;
        }
    }

    fn decrease(&mut self, elapsed_ms: u16) {
        if elapsed_ms > self.0 {
            self.0 = 0;
        } else {
            self.0 -= elapsed_ms;
        }
    }
}

/// Default per-key debouncer. The debouncing algorithm is same as ZMK's [default debouncer](https://github.com/zmkfirmware/zmk/blob/19613128b901723f7b78c136792d72e6ca7cf4fc/app/module/lib/zmk_debounce/debounce.c)
pub struct DefaultDebouncer<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> {
    last_ms: u32,
    counters: [[DebounceCounter; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> Default
    for DefaultDebouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> DefaultDebouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM> {
    /// Create a default debouncer
    pub fn new() -> Self {
        DefaultDebouncer {
            counters: [[DebounceCounter(0); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            last_ms: 0,
        }
    }
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> DebouncerTrait
    for DefaultDebouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Per-key debounce, same with zmk's debounce algorithm
    fn detect_change_with_debounce(
        &mut self,
        in_idx: usize,
        out_idx: usize,
        pin_state: bool,
        key_state: &KeyState,
    ) -> DebounceState {
        // Check debounce state every 1 ms
        let cur_ms = Instant::now().as_millis() as u32;
        let elapsed_ms = (cur_ms - self.last_ms) as u16;

        // If `elapsed_ms` == 0, the debounce state is checked within 1 ms, skip
        if elapsed_ms > 0 {
            let counter: &mut DebounceCounter = &mut self.counters[out_idx][in_idx];

            if key_state.pressed == pin_state {
                // If current key state matches input level, decrease debounce counter
                counter.decrease(elapsed_ms);
                // If there's no key change, the counter should always be 0.
                // So if the counter != 0, it's in a debouncing process
                if counter.0 > 0 {
                    DebounceState::InProgress
                } else {
                    DebounceState::Ignored
                }
            } else if counter.0 < DEBOUNCE_THRESHOLD {
                // If debounce threshold is not exceeded, increase debounce counter
                counter.increase(elapsed_ms);
                DebounceState::InProgress
            } else {
                // Debounce threshold is exceeded, reset counter
                self.last_ms = cur_ms;
                counter.0 = 0;
                DebounceState::Debounced
            }
        } else {
            DebounceState::Ignored
        }
    }
}
