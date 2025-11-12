use core::pin::pin;
use embassy_time::Timer;
use embedded_hal::digital::{ErrorType, InputPin, OutputPin};

#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::{Event, KeyboardEvent};
use crate::input_device::InputDevice;
use crate::matrix::{KeyState, MatrixTrait};

pub trait ActiveIn<const COL2ROW: bool>: ErrorType {
    fn is_active(&mut self) -> bool;
    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error>;
}

impl<
    #[cfg(feature = "async_matrix")]
    In: InputPin + Wait,
    #[cfg(not(feature = "async_matrix"))]
    In: InputPin,
    const COL2ROW: bool
> ActiveIn<COL2ROW> for In {
    fn is_active(&mut self) -> bool {
        if COL2ROW {
            self.is_low().ok().unwrap_or_default()
        } else {
            self.is_high().ok().unwrap_or_default()
        }
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_any_edge().await
    }
}

pub trait ActiveOut<const COL2ROW: bool> {
    fn deactivate(&mut self);
    fn activate(&mut self);
}

impl<Out: OutputPin, const COL2ROW: bool> ActiveOut<COL2ROW> for Out {
    fn deactivate(&mut self) {
        if COL2ROW {
            self.set_high().ok();
        } else {
            self.set_low().ok();
        }
    }

    fn activate(&mut self) {
        if COL2ROW {
            self.set_low().ok();
        } else {
            self.set_high().ok();
        }
    }
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct LowPowerMatrix<
    In: ActiveIn<COL2ROW>,
    Out: ActiveOut<COL2ROW>,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> {
    /// Row pins of the pcb matrix, are always input pins
    row_pins: [Out; ROW],
    /// Column pins of the pcb matrix, are always output pins
    col_pins: [In; COL],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; ROW]; COL],
    scan_pos: (usize, usize),
    in_progress: bool,
    any_key_pressed: bool,
}

impl<
    In: ActiveIn<COL2ROW>,
    Out: ActiveOut<COL2ROW>,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> LowPowerMatrix<In, Out, D, ROW, COL, COL2ROW>
{
    /// Create a matrix from input and output pins.
    pub fn new(row_pins: [Out; ROW], col_pins: [In; COL], debouncer: D) -> Self {
        LowPowerMatrix {
            row_pins,
            col_pins,
            debouncer,
            key_states: [[KeyState::new(); ROW]; COL],
            scan_pos: (0, 0),
            in_progress: false,
            any_key_pressed: false,
        }
    }

    fn get_key_event(&self, row_idx: usize, col_idx: usize) -> KeyboardEvent {
        KeyboardEvent::key(
            row_idx as u8,
            col_idx as u8,
            self.key_states[col_idx][row_idx].pressed,
        )
    }

    fn get_key_state(&self, row_idx: usize, col_idx: usize) -> KeyState {
        self.key_states[col_idx][row_idx]
    }

    fn toggle_key_state(&mut self, row_idx: usize, col_idx: usize) {
        self.key_states[col_idx][row_idx].toggle_pressed();
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_input_pins(&mut self) {
        let mut futs: Vec<_, COL> = self
            .col_pins
            .iter_mut()
            .map(|col_pin| col_pin.wait_for_any_edge())
            .collect();
        let _ = select_slice(pin!(futs.as_mut_slice())).await;
    }
}

impl<
    In: ActiveIn<COL2ROW>,
    Out: ActiveOut<COL2ROW>,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> InputDevice for LowPowerMatrix<In, Out, D, ROW, COL, COL2ROW>
{
    async fn read_event(&mut self) -> Event {
        loop {
            let (row_idx_start, col_idx_start) = self.scan_pos;

            for row_idx in row_idx_start..ROW {
                // Activate output pin, wait 1us ensuring the change comes into effect
                if let Some(row_pin) = self.row_pins.get_mut(row_idx) {
                    row_pin.activate();
                }
                Timer::after_micros(1).await;

                for col_idx in col_idx_start..COL {
                    let col_pin_state = if let Some(col_pin) = self.col_pins.get_mut(col_idx) {
                        col_pin.is_active()
                    } else {
                        false
                    };

                    // Check input pins and debounce
                    let debounce_state = self.debouncer.detect_change_with_debounce(
                        row_idx,
                        col_idx,
                        col_pin_state,
                        &self.get_key_state(row_idx, col_idx),
                    );

                    match debounce_state {
                        DebounceState::Debounced => {
                            self.toggle_key_state(row_idx, col_idx);
                            self.scan_pos = (row_idx, col_idx);
                            return Event::Key(self.get_key_event(row_idx, col_idx));
                        }
                        DebounceState::InProgress => {
                            self.in_progress = true;
                        }
                        DebounceState::Ignored => {
                            if self.get_key_state(row_idx, col_idx).pressed {
                                self.any_key_pressed = true;
                            }
                        }
                    }
                }

                // Deactivate pin
                if let Some(row_pin) = self.row_pins.get_mut(row_idx) {
                    row_pin.deactivate();
                }
            }

            if self.in_progress {
                // Sleep 1 ms before scanning again
                Timer::after_millis(1).await;
            } else if self.any_key_pressed {
                // Wait for any key change
                // Scan again after 10 ms to detect other keys in the same column as they are ghosted
                Timer::after_millis(10).await;
                // Todo: measure which approach is more efficient:
                // This might depend on the microcontroller and its pull-resistors
                // select(Timer::after_millis(10), self.wait_for_key()).await;
            } else {
                // Sleep until a key is pressed
                #[cfg(feature = "async_matrix")]
                self.wait_for_key().await;
                #[cfg(not(feature = "async_matrix"))]
                Timer::after_millis(10).await;
            }

            self.scan_pos = (0, 0);
            self.any_key_pressed = false;
            self.in_progress = false;
        }
    }
}

impl<
    In: ActiveIn<COL2ROW>,
    Out: ActiveOut<COL2ROW>,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> MatrixTrait<ROW, COL> for LowPowerMatrix<In, Out, D, ROW, COL, COL2ROW>
{
    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        // First, set activate row pins
        for row_pin in self.row_pins.iter_mut() {
            row_pin.activate();
        }
        Timer::after_micros(1).await;

        // Wait for any key press
        self.wait_input_pins().await;

        // Deactivate row pins
        for row_pin in self.row_pins.iter_mut() {
            row_pin.deactivate();
        }
    }
}
