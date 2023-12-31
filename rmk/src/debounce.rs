use crate::matrix::KeyState;
use rtic_monotonics::{systick::Systick, Monotonic};

/// Default DEBOUNCE_THRESHOLD in ms.
static DEBOUNCE_THRESHOLD: u16 = 10;

/// Debounce info for each key.
#[derive(Copy, Clone, Debug)]
pub struct DebounceState {
    pub counter: u16,
}

impl DebounceState {
    fn increase(&mut self, elapsed_ms: u16) {
        // Prevent overflow
        if u16::MAX - self.counter <= elapsed_ms {
            self.counter = u16::MAX;
        } else {
            self.counter += 1;
        }
    }

    fn decrease(&mut self, elapsed_ms: u16) {
        if elapsed_ms > self.counter {
            self.counter = 0;
        } else {
            self.counter -= 1;
        }
    }
}

/// Default per-key debouncer. The debouncing algorithm is same as ZMK's [default debouncer](https://github.com/zmkfirmware/zmk/blob/main/app/drivers/kscan/debounce.c)
pub struct Debouncer<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> {
    last_tick: u32,
    pub debounce_state: [[DebounceState; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize>
    Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Create a default debouncer
    pub fn new() -> Self {
        Debouncer {
            debounce_state: [[DebounceState { counter: 0 }; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            last_tick: 0,
        }
    }

    /// Per-key debounce, same with zmk's debounce algorithm
    pub fn debounce(
        &mut self,
        in_idx: usize,
        out_idx: usize,
        pin_state: bool,
        key_state: &mut KeyState,
    ) {
        // Record debounce state per ms
        let cur_tick = Systick::now().ticks();
        let elapsed_ms = (cur_tick - self.last_tick) as u16;

        if elapsed_ms > 0 {
            let state: &mut DebounceState = &mut self.debounce_state[out_idx][in_idx];

            key_state.changed = false;
            if key_state.pressed == pin_state {
                state.decrease(elapsed_ms);
                return;
            }

            // Use 10khz tick, so the debounce threshold should * 10
            if state.counter < DEBOUNCE_THRESHOLD * 10 {
                state.increase(elapsed_ms);
                return;
            }

            self.last_tick = cur_tick;
            state.counter = 0;
            key_state.pressed = !key_state.pressed;
            key_state.changed = true;
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
