use rtic_monotonics::{systick::Systick, Monotonic};

/// Default DEBOUNCE_TIME is 5ms.
static DEBOUNCE_TIME: u32 = 5;

/// Debounce info for each key.
#[derive(Copy, Clone)]
struct DebounceInfo {
    debouncing: bool,
    debounce_start: u32,
}

/// Default per-key debouncer. The debouncing algorithm is same as QMK's [default debouncer](https://github.com/qmk/qmk_firmware/blob/b2ded61796aee1f705a222e229c5b55416d93dd0/quantum/debounce/sym_defer_g.c#L33C16-L33C16)
pub struct Debouncer<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> {
    deboucing_info: [[DebounceInfo; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize>
    Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Create a default debouncer
    pub fn new() -> Self {
        Debouncer {
            deboucing_info: [[DebounceInfo {
                debouncing: false,
                debounce_start: 0,
            }; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
        }
    }

    /// Debounce given key. If the elapsed time from debounce starts is larger than DEBOUNCE_TIME, update the key state.
    pub fn debounce(
        &mut self,
        in_idx: usize,
        out_idx: usize,
        changed: bool,
        current_state: bool,
        raw_state: bool,
    ) -> (bool, bool) {
        if changed {
            self.deboucing_info[out_idx][in_idx].debouncing = true;
            self.deboucing_info[out_idx][in_idx].debounce_start = Systick::now().ticks();
            // FIXME: Possible overflow? 
            // Rust can handle overflow if the firmware is compiled with `--release`
        } else if self.deboucing_info[out_idx][in_idx].debouncing
            && (Systick::now().ticks() - self.deboucing_info[out_idx][in_idx].debounce_start)
                >= DEBOUNCE_TIME
        {
            self.deboucing_info[out_idx][in_idx].debouncing = false;
            self.deboucing_info[out_idx][in_idx].debounce_start = 0;
            if current_state != raw_state {
                // Debounced, return new state
                return (current_state, true);
            }
        }
        // Current key state stays unchanged
        (raw_state, false)
    }
}

impl<const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize> Default for Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM> {
    fn default() -> Self {
        Self::new()
    }
}
