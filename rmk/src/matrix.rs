use crate::debounce::Debouncer;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use defmt::Format;

/// KeyState represents the state of a key.
#[derive(Copy, Clone, Debug, Format)]
pub(crate) struct KeyState {
    // True if the key is pressed
    pub(crate) pressed: bool,
    // True if the key's state is just changed
    pub(crate) changed: bool,
    // If the key is held, `hold_start` records the time of it was pressed.
    hold_start: Option<Instant>,
}

impl Default for KeyState {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyState {
    fn new() -> Self {
        KeyState {
            pressed: false,
            changed: false,
            hold_start: None,
        }
    }

    // Record the start time of pressing
    fn start_timer(&mut self) {
        self.hold_start = Some(Instant::now());
    }

    // Calcuate held time
    fn elapsed(&self) -> Option<Duration> {
        match self.hold_start {
            Some(t) => Instant::now().checked_duration_since(t),
            None => None,
        }
    }

    // Clear held timer
    fn clear_timer(&mut self) {
        self.hold_start = None;
    }

    fn toggle_pressed(&mut self) {
        self.pressed = !self.pressed;
    }
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct Matrix<
    In: InputPin,
    Out: OutputPin,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
> {
    /// Input pins of the pcb matrix
    input_pins: [In; INPUT_PIN_NUM],
    /// Output pins of the pcb matrix
    output_pins: [Out; OUTPUT_PIN_NUM],
    /// Debouncer
    debouncer: Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>,
    /// Key state matrix
    key_states: [[KeyState; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<In: InputPin, Out: OutputPin, const INPUT_PIN_NUM: usize, const OUTPUT_PIN_NUM: usize>
    Matrix<In, Out, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Create a matrix from input and output pins.
    pub(crate) fn new(input_pins: [In; INPUT_PIN_NUM], output_pins: [Out; OUTPUT_PIN_NUM]) -> Self {
        Matrix {
            input_pins,
            output_pins,
            debouncer: Debouncer::new(),
            key_states: [[KeyState::new(); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
        }
    }

    /// Do matrix scanning, the result is stored in matrix's key_state field.
    pub(crate) async fn scan(&mut self) {
        for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
            // Pull up output pin, wait 1us ensuring the change comes into effect
            out_pin.set_high().ok();
            Timer::after_micros(1).await;
            for (in_idx, in_pin) in self.input_pins.iter_mut().enumerate() {
                // Check input pins and debounce
                let changed = self.debouncer.detect_change_with_debounce(
                    in_idx,
                    out_idx,
                    in_pin.is_high().ok().unwrap_or_default(),
                    &self.key_states[out_idx][in_idx],
                );

                if changed {
                    self.key_states[out_idx][in_idx].toggle_pressed();
                }

                self.key_states[out_idx][in_idx].changed = changed;
            }
            out_pin.set_low().ok();
        }
    }

    /// When a key is pressed, some callbacks some be called, such as `start_timer`
    pub(crate) fn key_pressed(&mut self, row: usize, col: usize) {
        // COL2ROW
        #[cfg(feature = "col2row")]
        self.key_states[col][row].start_timer();

        // ROW2COL
        #[cfg(not(feature = "col2row"))]
        self.key_states[row][col].start_timer();
    }

    /// Read key state at position (row, col)
    pub(crate) fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        // COL2ROW
        #[cfg(feature = "col2row")]
        return self.key_states[col][row];

        // ROW2COL
        #[cfg(not(feature = "col2row"))]
        return self.key_states[row][col];
    }
}
