#[cfg(feature = "async_matrix")]
use core::pin::pin;

use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
#[cfg(not(feature = "_ble"))]
use embedded_io_async::{Read, Write};
#[cfg(feature = "_ble")]
use {bt_hci::cmd::le::LeSetScanParams, bt_hci::controller::ControllerCmdSync, trouble_host::prelude::*};

use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::{Event, KeyEvent};
use crate::input_device::InputDevice;
use crate::matrix::{KeyState, MatrixTrait};

/// Run central's peripheral manager task.
///
/// # Arguments
/// * `id` - peripheral id
/// * `addr` - (optional) peripheral's BLE static address. This argument is enabled only for nRF BLE split now
/// * `receiver` - (optional) serial port. This argument is enabled only for serial split now
pub async fn run_peripheral_manager<
    'a,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    #[cfg(feature = "_ble")] C: Controller + ControllerCmdSync<LeSetScanParams>,
    #[cfg(not(feature = "_ble"))] S: Read + Write,
>(
    id: usize,
    #[cfg(feature = "_ble")] addr: Option<[u8; 6]>,
    #[cfg(feature = "_ble")] stack: &'a Stack<'a, C>,
    #[cfg(not(feature = "_ble"))] receiver: S,
) {
    #[cfg(feature = "_ble")]
    {
        use crate::split::ble::central::run_ble_peripheral_manager;
        run_ble_peripheral_manager::<C, ROW, COL, ROW_OFFSET, COL_OFFSET>(id, addr, stack).await;
    };

    #[cfg(not(feature = "_ble"))]
    {
        use crate::split::serial::run_serial_peripheral_manager;
        run_serial_peripheral_manager::<ROW, COL, ROW_OFFSET, COL_OFFSET, S>(id, receiver).await;
    };
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct CentralMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
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
    key_states: [[KeyState; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
    /// Start scanning
    scan_start: Option<Instant>,
    /// Current scan pos: (out_idx, in_idx)
    scan_pos: (usize, usize),
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > InputDevice for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    async fn read_event(&mut self) -> Event {
        loop {
            let (out_idx_start, in_idx_start) = self.scan_pos;

            #[cfg(feature = "async_matrix")]
            self.wait_for_key().await;

            // Scan matrix and send report
            for out_idx in out_idx_start..self.output_pins.len() {
                // Pull up output pin, wait 1us ensuring the change comes into effect
                if let Some(out_pin) = self.output_pins.get_mut(out_idx) {
                    out_pin.set_high().ok();
                }
                Timer::after_micros(1).await;
                for in_idx in in_idx_start..self.input_pins.len() {
                    let in_pin = self.input_pins.get_mut(in_idx).unwrap();
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
                            let (row, col, key_state) = (
                                (in_idx + ROW_OFFSET) as u8,
                                (out_idx + COL_OFFSET) as u8,
                                self.key_states[out_idx][in_idx],
                            );
                            #[cfg(not(feature = "col2row"))]
                            let (row, col, key_state) = (
                                (out_idx + ROW_OFFSET) as u8,
                                (in_idx + COL_OFFSET) as u8,
                                self.key_states[out_idx][in_idx],
                            );

                            self.scan_pos = (out_idx, in_idx);
                            return Event::Key(KeyEvent {
                                row,
                                col,
                                pressed: key_state.pressed,
                            });
                        }
                        _ => (),
                    }

                    // If there's key still pressed, always refresh the self.scan_start
                    #[cfg(feature = "async_matrix")]
                    if self.key_states[out_idx][in_idx].pressed {
                        self.scan_start = Some(Instant::now());
                    }
                }
                // Pull it back to low
                if let Some(out_pin) = self.output_pins.get_mut(out_idx) {
                    out_pin.set_low().ok();
                }
            }

            self.scan_pos = (0, 0);
            embassy_time::Timer::after_micros(100).await;
        }
    }
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > MatrixTrait for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    #[cfg(feature = "col2row")]
    const ROW: usize = INPUT_PIN_NUM;
    #[cfg(feature = "col2row")]
    const COL: usize = OUTPUT_PIN_NUM;
    #[cfg(not(feature = "col2row"))]
    const ROW: usize = OUTPUT_PIN_NUM;
    #[cfg(not(feature = "col2row"))]
    const COL: usize = INPUT_PIN_NUM;

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        use embassy_futures::select::select_slice;
        use heapless::Vec;

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
        let _ = select_slice(pin!(futs.as_mut_slice())).await;

        // Set all output pins back to low
        for out in self.output_pins.iter_mut() {
            out.set_low().ok();
        }

        self.scan_start = Some(Instant::now());
    }
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Initialization of central
    pub fn new(input_pins: [In; INPUT_PIN_NUM], output_pins: [Out; OUTPUT_PIN_NUM], debouncer: D) -> Self {
        CentralMatrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: [[KeyState::default(); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            scan_start: None,
            scan_pos: (0, 0),
        }
    }
}

/// DirectPinMartex only has input pins.
pub struct CentralDirectPinMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    D: DebouncerTrait,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
    const SIZE: usize,
> {
    /// Input pins of the pcb matrix
    direct_pins: [[Option<In>; COL]; ROW],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; COL]; ROW],
    /// Start scanning
    scan_start: Option<Instant>,
    /// Pin active level
    low_active: bool,
    /// Current scan pos: (out_idx, in_idx)
    scan_pos: (usize, usize),
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const ROW: usize,
        const COL: usize,
        const SIZE: usize,
    > CentralDirectPinMatrix<In, D, ROW_OFFSET, COL_OFFSET, ROW, COL, SIZE>
{
    /// Create a matrix from input and output pins.
    pub fn new(direct_pins: [[Option<In>; COL]; ROW], debouncer: D, low_active: bool) -> Self {
        CentralDirectPinMatrix {
            direct_pins,
            debouncer,
            key_states: [[KeyState::new(); COL]; ROW],
            scan_start: None,
            low_active,
            scan_pos: (0, 0),
        }
    }
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const ROW: usize,
        const COL: usize,
        const SIZE: usize,
    > InputDevice for CentralDirectPinMatrix<In, D, ROW_OFFSET, COL_OFFSET, ROW, COL, SIZE>
{
    async fn read_event(&mut self) -> Event {
        info!("Central Direct Pin Matrix scanning");
        loop {
            let (row_idx_start, col_idx_start) = self.scan_pos;

            #[cfg(feature = "async_matrix")]
            self.wait_for_key().await;

            // Scan matrix and send report
            for row_idx in row_idx_start..self.direct_pins.len() {
                let pins_row = self.direct_pins.get_mut(row_idx).unwrap();
                for col_idx in col_idx_start..pins_row.len() {
                    let direct_pin = pins_row.get_mut(col_idx).unwrap();
                    if let Some(direct_pin) = direct_pin {
                        let pin_state = if self.low_active {
                            direct_pin.is_low().ok().unwrap_or_default()
                        } else {
                            direct_pin.is_high().ok().unwrap_or_default()
                        };

                        let debounce_state = self.debouncer.detect_change_with_debounce(
                            col_idx,
                            row_idx,
                            pin_state,
                            &self.key_states[row_idx][col_idx],
                        );

                        match debounce_state {
                            DebounceState::Debounced => {
                                self.key_states[row_idx][col_idx].toggle_pressed();
                                let (col, row, key_state) = (
                                    (col_idx + COL_OFFSET) as u8,
                                    (row_idx + ROW_OFFSET) as u8,
                                    self.key_states[row_idx][col_idx],
                                );

                                self.scan_pos = (row_idx, col_idx);
                                return Event::Key(KeyEvent {
                                    row,
                                    col,
                                    pressed: key_state.pressed,
                                });
                            }
                            _ => (),
                        }

                        // If there's key still pressed, always refresh the self.scan_start
                        #[cfg(feature = "async_matrix")]
                        if self.key_states[row_idx][col_idx].pressed {
                            self.scan_start = Some(Instant::now());
                        }
                    }
                }
            }

            self.scan_pos = (0, 0);
            Timer::after_micros(100).await;
        }
    }
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const ROW: usize,
        const COL: usize,
        const SIZE: usize,
    > MatrixTrait for CentralDirectPinMatrix<In, D, ROW_OFFSET, COL_OFFSET, ROW, COL, SIZE>
{
    const ROW: usize = ROW;
    const COL: usize = COL;

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        use embassy_futures::select::select_slice;
        use heapless::Vec;
        if let Some(start_time) = self.scan_start {
            // If no key press over 1ms, stop scanning and wait for interrupt
            if start_time.elapsed().as_millis() <= 1 {
                return;
            } else {
                self.scan_start = None;
            }
        }
        Timer::after_micros(1).await;
        info!("Waiting for active level");

        if self.low_active {
            let mut futs: Vec<_, SIZE> = Vec::new();
            for direct_pins_row in self.direct_pins.iter_mut() {
                for direct_pin in direct_pins_row.iter_mut() {
                    if let Some(direct_pin) = direct_pin {
                        let _ = futs.push(direct_pin.wait_for_low());
                    }
                }
            }
            let _ = select_slice(pin!(futs.as_mut_slice())).await;
        } else {
            let mut futs: Vec<_, SIZE> = Vec::new();
            for direct_pins_row in self.direct_pins.iter_mut() {
                for direct_pin in direct_pins_row.iter_mut() {
                    if let Some(direct_pin) = direct_pin {
                        let _ = futs.push(direct_pin.wait_for_high());
                    }
                }
            }
            let _ = select_slice(pin!(futs.as_mut_slice())).await;
        }
        self.scan_start = Some(Instant::now());
    }
}
