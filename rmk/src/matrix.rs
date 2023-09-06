use crate::debounce::Debouncer;
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use rtic_monotonics::systick::*;

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
    cur_key_state: [[bool; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
    pub key_state: [[bool; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
    pub changed: [[bool; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
    debouncer: Debouncer<INPUT_PIN_NUM, OUTPUT_PIN_NUM>,
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
            cur_key_state: [[false; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            key_state: [[false; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            changed: [[false; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
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
            for (in_idx, pin) in self.input_pins.iter().enumerate() {
                let mut key_changed = false;
                if pin.is_high()? ^ self.cur_key_state[out_idx][in_idx] {
                    self.cur_key_state[out_idx][in_idx] = pin.is_high()?;
                    key_changed = true;
                }

                // Debounce, update key state if there's a key changed after debounc
                (
                    self.key_state[out_idx][in_idx],
                    self.changed[out_idx][in_idx],
                ) = self.debouncer.debounce(
                    in_idx,
                    out_idx,
                    key_changed,
                    self.cur_key_state[out_idx][in_idx],
                    self.key_state[out_idx][in_idx],
                );
            }
            out_pin.set_low()?;
        }
        Ok(())
    }
}
