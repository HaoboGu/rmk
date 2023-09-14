use crate::debounce::Debouncer;
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use rtic_monotonics::{
    systick::{fugit::{Instant, Duration}, *},
    Monotonic,
};

/// KeyState represents the state of a key.
#[derive(Copy, Clone, Debug)]
pub struct KeyState {
    pub pressed: bool,
    pub changed: bool,
    pub hold_start: Option<Instant<u32, 1, 1000>>,
}

impl KeyState {
    pub fn new() -> Self {
        KeyState {
            pressed: false,
            changed: false,
            hold_start: None,
        }
    }

    pub fn start_timer(&mut self) {
        self.hold_start = Some(Systick::now());
    }

    pub fn elapsed(&self) -> Option<Duration<u32, 1, 1000>> {
        match self.hold_start {
            Some(t) => Systick::now().checked_duration_since(t),
            None => None,
        }
    }

    pub fn clear_timer(&mut self) {
        self.hold_start = None;
    }
}

/// Key's position in the matrix
pub struct KeyPos {
    row: u8,
    col: u8,
}

/// Matrix is the physical pcb layout of the keyboard matrix.
///
pub struct Matrix<
    In: InputPin,
    Out: OutputPin,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
> {
    input_pins: [In; INPUT_PIN_NUM],
    output_pins: [Out; OUTPUT_PIN_NUM],
    pub debouncer: Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>,
    /// Key state matrix
    pub key_states: [[KeyState; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
}

impl<
        In: InputPin<Error = Infallible>,
        Out: OutputPin<Error = Infallible>,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > Matrix<In, Out, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Create a matrix from input and output pins.
    pub fn new(input_pins: [In; INPUT_PIN_NUM], output_pins: [Out; OUTPUT_PIN_NUM]) -> Self {
        Matrix {
            input_pins,
            output_pins,
            debouncer: Debouncer::new(),
            key_states: [[KeyState::new(); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
        }
    }

    /// Do matrix scanning, the result is stored in matrix's key_state field.
    pub async fn scan(&mut self) -> Result<(), Infallible> {
        for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
            // Pull up output pin
            out_pin.set_high()?;
            Systick::delay(1.micros()).await;
            // Check input pins
            for (in_idx, in_pin) in self.input_pins.iter().enumerate() {
                // Debounce, update key state if there's a key changed after debounc
                self.debouncer.debounce(
                    in_idx,
                    out_idx,
                    in_pin.is_high()?,
                    &mut self.key_states[out_idx][in_idx],
                );
            }
            out_pin.set_low()?;
        }
        Ok(())
    }
}
