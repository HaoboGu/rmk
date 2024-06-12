use crate::debounce::{DebounceState, DebouncerTrait};
use defmt::Format;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use {
    defmt::info, embassy_futures::select::select_slice, embedded_hal_async::digital::Wait,
    heapless::Vec,
};

/// KeyState represents the state of a key.
#[derive(Copy, Clone, Debug, Format)]
pub(crate) struct KeyState {
    // True if the key is pressed
    pub(crate) pressed: bool,
    // True if the key's state is just changed
    pub(crate) changed: bool,
    // If the key is held, `hold_start` records the time of it was pressed.
    pub(crate) hold_start: Option<Instant>,
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

    // Calculate held time
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
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
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
    key_states: [[KeyState; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
    /// Start scanning
    scan_start: Option<Instant>,
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > Matrix<In, Out, D, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Create a matrix from input and output pins.
    pub(crate) fn new(input_pins: [In; INPUT_PIN_NUM], output_pins: [Out; OUTPUT_PIN_NUM]) -> Self {
        Matrix {
            input_pins,
            output_pins,
            debouncer: D::new(),
            key_states: [[KeyState::new(); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            scan_start: None,
        }
    }

    #[cfg(feature = "async_matrix")]
    pub(crate) async fn wait_for_key(&mut self) {
        if let Some(start_time) = self.scan_start {
            // If not key over 2 secs, wait for interupt in next loop
            if start_time.elapsed().as_secs() < 1 {
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
        info!("Waiting for high");
        let mut futs: Vec<_, INPUT_PIN_NUM> = self
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
    pub(crate) async fn scan(&mut self) {
        for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
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
                        self.key_states[out_idx][in_idx].changed = true;
                    }
                    _ => self.key_states[out_idx][in_idx].changed = false,
                }

                // If there's key changed or pressed, always refresh the self.scan_start
                #[cfg(feature = "async_matrix")]
                if self.key_states[out_idx][in_idx].changed
                    || self.key_states[out_idx][in_idx].pressed
                {
                    self.scan_start = Some(Instant::now());
                }
            }
            out_pin.set_low().ok();
        }
    }

    /// When a key is pressed, some callbacks some be called, such as `start_timer`
    pub(crate) fn update_timer(&mut self, row: usize, col: usize) {
        #[cfg(feature = "col2row")]
        let ks = &mut self.key_states[col][row];
        #[cfg(not(feature = "col2row"))]
        let ks = &mut self.key_states[row][col];

        if ks.pressed {
            ks.start_timer();
        } else {
            ks.clear_timer()
        }
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
