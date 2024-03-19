use embassy_time::Instant;

use crate::matrix::KeyState;

/// Default DEBOUNCE_THRESHOLD in ms.
static DEBOUNCE_THRESHOLD: u16 = 10;

/// Debounce counter info for each key.
#[derive(Copy, Clone, Debug)]
struct DebounceCounter(u16);

impl DebounceCounter {
    fn increase(&mut self, elapsed_ms: u16) {
        // Prevent overflow
        if u16::MAX - self.0 <= elapsed_ms {
            self.0 = u16::MAX;
        } else {
            self.0 += 1;
        }
    }

    fn decrease(&mut self, elapsed_ms: u16) {
        if elapsed_ms > self.0 {
            self.0 = 0;
        } else {
            self.0 -= 1;
        }
    }
}

/// Debounce state
pub(crate) enum DebounceState {
    Debounced,
    InProgress,
    HeldHigh,
    HeldLow,
    Ignored,
}

/// Default per-key debouncer. The debouncing algorithm is same as ZMK's [default debouncer](https://github.com/zmkfirmware/zmk/blob/19613128b901723f7b78c136792d72e6ca7cf4fc/app/module/lib/zmk_debounce/debounce.c)
pub(crate) struct Debouncer<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> {
    last_tick: u32,
    counters: [[DebounceCounter; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize>
    Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Create a default debouncer
    pub(crate) fn new() -> Self {
        Debouncer {
            counters: [[DebounceCounter(0); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            last_tick: 0,
        }
    }

    /// Per-key debounce, same with zmk's debounce algorithm
    pub(crate) fn detect_change_with_debounce(
        &mut self,
        in_idx: usize,
        out_idx: usize,
        pin_state: bool,
        key_state: &KeyState,
    ) -> DebounceState {
        // Use 10khz tick, so the debounce threshold should * 10
        const TICK_FREQUENCY: u16 = 10;

        // Record debounce state per ms
        let cur_tick = Instant::now().as_ticks() as u32;
        let elapsed_ms = (cur_tick - self.last_tick) as u16;

        if elapsed_ms > 0 {
            let counter: &mut DebounceCounter = &mut self.counters[out_idx][in_idx];

            if key_state.pressed == pin_state {
                // If current key state matches input pin, decrease debounce counter
                counter.decrease(elapsed_ms);
                if pin_state {
                    DebounceState::HeldHigh
                } else {
                    DebounceState::HeldLow
                }
            } else if counter.0 < DEBOUNCE_THRESHOLD * TICK_FREQUENCY {
                // If debounce threshold is not exceeded, increase debounce counter
                counter.increase(elapsed_ms);
                DebounceState::InProgress
            } else {
                // Debounce threshold is exceeded, reset counter
                self.last_tick = cur_tick;
                counter.0 = 0;
                DebounceState::Debounced
            }
        } else {
            DebounceState::Ignored
        }
    }
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> Default
    for Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    fn default() -> Self {
        Self::new()
    }
}
