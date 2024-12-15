use crate::{
    debounce::{DebounceState, DebouncerTrait},
    keyboard::{key_event_channel, KeyEvent},
};
use defmt::{error, Format};
use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use generic_array::{sequence::GenericSequence, ArrayLength, GenericArray};
use typenum::{NonZero, Unsigned};
#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

/// MatrixTrait is the trait for keyboard matrix.
///
/// The keyboard matrix is a 2D matrix of keys, the matrix does the scanning and saves the result to each key's `KeyState`.
/// The `KeyState` at position (row, col) can be read by `get_key_state` and updated by `update_key_state`.
pub(crate) trait MatrixTrait {
    // Matrix size
    type Row: Unsigned + NonZero;
    type Col: Unsigned + NonZero;

    // Do matrix scanning, save the result in matrix's key_state field.
    async fn scan(&mut self);
    // Read key state at position (row, col)
    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState;
    // Update key state at position (row, col)
    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState));
    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self);
}

/// KeyState represents the state of a key.
#[derive(Copy, Clone, Debug, Format)]
pub(crate) struct KeyState {
    // True if the key is pressed
    pub(crate) pressed: bool,
    // True if the key's state is just changed
    // pub(crate) changed: bool,
}

impl Default for KeyState {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyState {
    pub(crate) fn new() -> Self {
        KeyState { pressed: false }
    }

    pub(crate) fn toggle_pressed(&mut self) {
        self.pressed = !self.pressed;
    }

    pub(crate) fn is_releasing(&self) -> bool {
        !self.pressed
    }

    pub(crate) fn is_pressing(&self) -> bool {
        self.pressed
    }
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub(crate) struct Matrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    InputPinNum: ArrayLength + NonZero,
    OutputPinNum: ArrayLength + NonZero,
> {
    /// Input pins of the pcb matrix
    input_pins: GenericArray<In, InputPinNum>,
    /// Output pins of the pcb matrix
    output_pins: GenericArray<Out, OutputPinNum>,
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: GenericArray<GenericArray<KeyState, InputPinNum>, OutputPinNum>,
    /// Start scanning
    scan_start: Option<Instant>,
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        InputPinNum: ArrayLength + NonZero,
        OutputPinNum: ArrayLength + NonZero,
    > Matrix<In, Out, D, InputPinNum, OutputPinNum>
{
    /// Create a matrix from input and output pins.
    pub(crate) fn new(
        input_pins: GenericArray<In, InputPinNum>,
        output_pins: GenericArray<Out, OutputPinNum>,
        debouncer: D,
    ) -> Self {
        Matrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: GenericArray::generate(|_| GenericArray::generate(|_| KeyState::new())),
            scan_start: None,
        }
    }
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        InputPinNum: ArrayLength + NonZero,
        OutputPinNum: ArrayLength + NonZero,
    > MatrixTrait for Matrix<In, Out, D, InputPinNum, OutputPinNum>
{
    #[cfg(feature = "col2row")]
    type Row = InputPinNum;
    #[cfg(feature = "col2row")]
    type Col = OutputPinNum;
    #[cfg(not(feature = "col2row"))]
    type Row = OutputPinNum;
    #[cfg(not(feature = "col2row"))]
    type Col = InputPinNum;

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        if let Some(start_time) = self.scan_start {
            // If no key press over 1ms, stop scanning and wait for interupt
            if start_time.elapsed().as_millis() <= 1 {
                return;
            } else {
                self.scan_start = None;
            }
        }
        // First, set all output pin to high
        for out in self.output_pins.iter_mut() {
            out.set_high().ok();
        }
        Timer::after_micros(1).await;
        let mut futs: Vec<_, InputPinNum> = self
            .input_pins
            .iter_mut()
            .map(|input_pin| input_pin.wait_for_high())
            .collect();
        let _ = select_slice(futs.as_mut_slice()).await;

        // Set all output pins back to low
        for out in self.output_pins.iter_mut() {
            out.set_low().ok();
        }

        self.scan_start = Some(Instant::now());
    }

    /// Do matrix scanning, the result is stored in matrix's key_state field.
    async fn scan(&mut self) {
        defmt::info!("Matrix scanning");
        loop {
            #[cfg(feature = "async_matrix")]
            self.wait_for_key().await;

            // Scan matrix and send report
            for (out_idx, out_pin) in self.output_pins.as_mut_slice().iter_mut().enumerate() {
                // Pull up output pin, wait 1us ensuring the change comes into effect
                out_pin.set_high().ok();
                Timer::after_micros(1).await;
                for (in_idx, in_pin) in self.input_pins.iter_mut().enumerate() {
                    // Check input pins and debounce
                    let debounce_state = self.debouncer.detect_change_with_debounce(
                        in_idx,
                        out_idx,
                        in_pin.is_high().ok().unwrap_or_default(),
                        &self.key_states[out_idx][in_idx],
                    );

                    match debounce_state {
                        DebounceState::Debounced => {
                            self.key_states[out_idx][in_idx].toggle_pressed();
                            #[cfg(feature = "col2row")]
                            let (row, col, key_state) =
                                (in_idx, out_idx, self.key_states[out_idx][in_idx]);
                            #[cfg(not(feature = "col2row"))]
                            let (row, col, key_state) =
                                (out_idx, in_idx, self.key_states[out_idx][in_idx]);

                            // `try_send` is used here because we don't want to block scanning if the channel is full
                            let send_re = key_event_channel.try_send(KeyEvent {
                                row: row as u8,
                                col: col as u8,
                                pressed: key_state.pressed,
                            });
                            if send_re.is_err() {
                                error!("Failed to send key event: key event channel full");
                            }
                        }
                        _ => (),
                    }

                    // If there's key still pressed, always refresh the self.scan_start
                    #[cfg(feature = "async_matrix")]
                    if self.key_states[out_idx][in_idx].pressed {
                        self.scan_start = Some(Instant::now());
                    }
                }
                out_pin.set_low().ok();
            }

            embassy_time::Timer::after_micros(100).await;
        }
    }

    /// Read key state at position (row, col)
    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        // COL2ROW
        #[cfg(feature = "col2row")]
        return self.key_states[col][row];

        // ROW2COL
        #[cfg(not(feature = "col2row"))]
        return self.key_states[row][col];
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        // COL2ROW
        #[cfg(feature = "col2row")]
        f(&mut self.key_states[col][row]);

        // ROW2COL
        #[cfg(not(feature = "col2row"))]
        f(&mut self.key_states[row][col]);
    }
}
