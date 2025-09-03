use embassy_time::{Instant, Timer};
#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

use crate::{debounce::{DebounceState, DebouncerTrait}, matrix::{KeyState, MatrixTrait}};
use crate::event::{Event, KeyboardEvent};
use crate::input_device::InputDevice;
use crate::driver::flex_pin::FlexPin;

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct BidirectionalMatrix<
    #[cfg(not(feature = "async_matrix"))] In: FlexPin,
    #[cfg(feature = "async_matrix")] In: Wait + FlexPin,
    #[cfg(not(feature = "async_matrix"))] Out: FlexPin,
    #[cfg(feature = "async_matrix")] Out: Wait + FlexPin,
    D: DebouncerTrait,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
    const COLS: usize,
> {
    /// Input pins of the pcb matrix
    input_pins: [In; INPUT_PIN_NUM],
    /// Output pins of the pcb matrix
    output_pins: [Out; OUTPUT_PIN_NUM],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_state: [[KeyState; INPUT_PIN_NUM]; COLS],
    /// Start scanning
    scan_start: Option<Instant>,
    /// Current scan pos: (out_idx, in_idx)
    scan_pos: (usize, usize),
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: FlexPin,
    #[cfg(feature = "async_matrix")] In: Wait + FlexPin,
    #[cfg(not(feature = "async_matrix"))] Out: FlexPin,
    #[cfg(feature = "async_matrix")] Out: Wait + FlexPin,
    D: DebouncerTrait,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
    const COLS: usize,
> BidirectionalMatrix<In, Out, D, INPUT_PIN_NUM, OUTPUT_PIN_NUM, COLS>
{
    /// Create a matrix from input and output pins.
    pub fn new(input_pins: [In; INPUT_PIN_NUM], output_pins: [Out; OUTPUT_PIN_NUM], debouncer: D) -> Self {
        BidirectionalMatrix {
            input_pins,
            output_pins,
            debouncer,
            key_state: [[KeyState::new(); INPUT_PIN_NUM]; COLS],
            scan_start: None,
            scan_pos: (0, 0),
        }
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: FlexPin,
    #[cfg(feature = "async_matrix")] In: Wait + FlexPin,
    #[cfg(not(feature = "async_matrix"))] Out: FlexPin,
    #[cfg(feature = "async_matrix")] Out: Wait + FlexPin,
    D: DebouncerTrait,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
    const COLS: usize,
> InputDevice for BidirectionalMatrix<In, Out, D, INPUT_PIN_NUM, OUTPUT_PIN_NUM, COLS>
{
    async fn read_event(&mut self) -> crate::event::Event {
        loop {
            let (mut out_idx_start, mut in_idx_start) = self.scan_pos;
            #[cfg(feature = "async_matrix")]
            self.wait_for_key().await;
            
            if out_idx_start < OUTPUT_PIN_NUM {                
                // Scan matrix and send report
                for out_idx in out_idx_start..self.output_pins.len() {
                    // Pull up output pin, wait 1us ensuring the change comes into effect
                    if let Some(out_pin) = self.output_pins.get_mut(out_idx) {
                        out_pin.set_high().ok();
                    }
                    Timer::after_micros(1).await;
                    let in_idx_start_current = if out_idx == out_idx_start { in_idx_start } else { 0 };
                    for in_idx in in_idx_start_current..self.input_pins.len() {
                        let in_pin = self.input_pins.get_mut(in_idx).unwrap();
                        // Check input pins and debounce
                        let debounce_state = self.debouncer.detect_change_with_debounce(
                            in_idx,
                            out_idx,
                            in_pin.is_high().ok().unwrap_or_default(),
                            &self.key_state[out_idx][in_idx],
                        );
    
                        if let DebounceState::Debounced = debounce_state {
                            self.key_state[out_idx][in_idx].toggle_pressed();
    
                            self.scan_pos = (out_idx, in_idx);
                            return Event::Key(KeyboardEvent::key(in_idx as u8, (out_idx * 2) as u8, self.key_state[out_idx][in_idx].pressed));
                        }
    
                        // If there's key still pressed, always refresh the self.scan_start
                        #[cfg(feature = "async_matrix")]
                        if self.key_state[out_idx][in_idx].pressed {
                            self.scan_start = Some(Instant::now());
                        }
                    }
    
                    // Pull it back to low
                    if let Some(out_pin) = self.output_pins.get_mut(out_idx) {
                        out_pin.set_low().ok();
                    }
                }
                out_idx_start = OUTPUT_PIN_NUM;
                in_idx_start = 0;
            }

            // Set all output pins back to low
            for out in self.output_pins.iter_mut() {
                out.set_low().ok();
            }
            
            let local_in_idx_start = out_idx_start - OUTPUT_PIN_NUM;
            let local_out_idx_start = in_idx_start;
            // Scan matrix in reverse and send report
            for in_idx in local_in_idx_start..self.input_pins.len() {
                // Pull up output pin, wait 1us ensuring the change comes into effect
                if let Some(in_pin) = self.input_pins.get_mut(in_idx) {
                    in_pin.set_high().ok();
                }
                Timer::after_micros(1).await;
                // Only use start index for the first iteration.
                let out_idx_start_current = if in_idx == local_in_idx_start { local_out_idx_start } else { 0 };
                for out_idx in out_idx_start_current..self.output_pins.len() {
                    let col_idx = out_idx + OUTPUT_PIN_NUM;
                    let out_pin = self.input_pins.get_mut(out_idx).unwrap();
                    // Check input pins and debounce
                    let debounce_state = self.debouncer.detect_change_with_debounce(
                        col_idx,
                        in_idx,
                        out_pin.is_high().ok().unwrap_or_default(),
                        &self.key_state[col_idx][in_idx],
                    );

                    if let DebounceState::Debounced = debounce_state {
                        self.key_state[col_idx][in_idx].toggle_pressed();
                        self.scan_pos = (in_idx, col_idx);
                        return Event::Key(KeyboardEvent::key(in_idx as u8, (out_idx * 2 + 1) as u8, self.key_state[col_idx][in_idx].pressed));
                    }

                    // If there's key still pressed, always refresh the self.scan_start
                    #[cfg(feature = "async_matrix")]
                    if self.key_state[out_idx][in_idx].pressed {
                        self.scan_start = Some(Instant::now());
                    }
                }

                // Pull it back to low
                if let Some(in_pin) = self.output_pins.get_mut(in_idx) {
                    in_pin.set_low().ok();
                }
            }
            self.scan_pos = (0, 0);
        }
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: FlexPin,
    #[cfg(feature = "async_matrix")] In: Wait + FlexPin,
    #[cfg(not(feature = "async_matrix"))] Out: FlexPin,
    #[cfg(feature = "async_matrix")] Out: Wait + FlexPin,
    D: DebouncerTrait,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
    const COLS: usize
> MatrixTrait for BidirectionalMatrix<In, Out, D, INPUT_PIN_NUM, OUTPUT_PIN_NUM, COLS>
{
    const ROW: usize = INPUT_PIN_NUM;
    const COL: usize = COLS;

    #[cfg(feature = "async_matrix")]
    // @TODO: Update for bidirectional scanning.
    async fn wait_for_key(&mut self) {
        use core::pin::pin;

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
        let mut futs: Vec<_, INPUT_PIN_NUM> = self
            .input_pins
            .iter_mut()
            .map(|input_pin| input_pin.wait_for_high())
            .collect();
        let _ = select_slice(pin!(futs.as_mut_slice())).await;

        // Set all output pins back to low
        for out in self.output_pins.iter_mut() {
            out.set_low().ok();
        }

        self.scan_start = Some(Instant::now());
    }
}
