use rtic_monotonics::{systick::Systick, Monotonic};

/// Default DEBOUNCE_THRESHOLD.
static DEBOUNCE_THRESHOLD: u16 = 5;

/// Debounce info for each key.
#[derive(Copy, Clone, Debug)]
pub struct DebounceState {
    pub pressed: bool,
    pub changed: bool,
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
    pub key_state: [[DebounceState; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize>
    Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Create a default debouncer
    pub fn new() -> Self {
        Debouncer {
            key_state: [[DebounceState {
                pressed: false,
                changed: false,
                counter: 0,
            }; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            last_tick: 0,
        }
    }

    /// Per-key debounce, same with zmk's debounce algorithm
    pub fn debounce(&mut self, in_idx: usize, out_idx: usize, pressed: bool) {
        // Record debounce state per ms
        let cur_tick = Systick::now().ticks();
        let elapsed_ms = (cur_tick - self.last_tick) as u16;

        if elapsed_ms > 0 {
            let state: &mut DebounceState = &mut self.key_state[out_idx][in_idx];

            state.changed = false;
            if state.pressed == pressed {
                state.decrease(elapsed_ms);
                return;
            }

            if state.counter < DEBOUNCE_THRESHOLD {
                state.increase(elapsed_ms);
                return;
            }

            state.pressed = !state.pressed;
            state.counter = 0;
            state.changed = true;
            self.last_tick = cur_tick;
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
