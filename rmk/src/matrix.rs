use crate::debounce::Debouncer;
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use rtic_monotonics::systick::*;

/// KeyState represents the state of a key.
#[derive(Copy, Clone, Debug)]
pub struct KeyState {
    pub pressed: bool,
    pub changed: bool,
}

impl KeyState {
    pub fn new() -> Self {
        KeyState {
            pressed: false,
            changed: false,
        }
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
                self.debouncer.debounce(in_idx, out_idx, in_pin.is_high()?);
            }
            out_pin.set_low()?;
        }
        Ok(())
    }
}
