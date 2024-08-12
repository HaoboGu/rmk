use embassy_time::Instant;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;

use crate::{debounce::DebouncerTrait, matrix::KeyState};

pub struct SpiltSender {}

impl SpiltSender {
    pub(crate) fn send(&self) {}
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct SlaveMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
> {
    /// Input pins of the pcb matrix
    input_pins: [In; INPUT_PIN_NUM],
    /// Output pins of the pcb matrix
    output_pins: [Out; OUTPUT_PIN_NUM],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; COL]; ROW],
    /// Start scanning
    scan_start: Option<Instant>,
    /// Sender
    sender: SpiltSender,
}
