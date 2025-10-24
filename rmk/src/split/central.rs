#[cfg(feature = "_ble")]
use core::cell::RefCell;
#[cfg(feature = "async_matrix")]
use core::pin::pin;

use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(not(feature = "_ble"))]
use embedded_io_async::{Read, Write};
#[cfg(feature = "_ble")]
use {
    bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy, LeSetScanParams},
    bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync},
    heapless::VecView,
    trouble_host::prelude::*,
};
#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::{Event, KeyboardEvent};
use crate::input_device::InputDevice;
use crate::matrix::{ColPins, KeyState, MatrixInputPins, MatrixOutputPins, MatrixTrait, RowPins};

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
    #[cfg(feature = "_ble")] C: Controller
        + ControllerCmdSync<LeSetScanParams>
        + ControllerCmdAsync<LeSetPhy>
        + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(not(feature = "_ble"))] S: Read + Write,
>(
    id: usize,
    #[cfg(feature = "_ble")] addr: &RefCell<VecView<Option<[u8; 6]>>>,
    #[cfg(feature = "_ble")] stack: &'a Stack<'a, C, DefaultPacketPool>,
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
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> where
    Self: RowPins<COL2ROW>,
    Self: ColPins<COL2ROW>,
{
    /// Row pins of the pcb matrix
    row_pins: <Self as RowPins<COL2ROW>>::RowPinsType,
    /// Column pins of the pcb matrix
    col_pins: <Self as ColPins<COL2ROW>>::ColPinsType,
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; ROW]; COL],
    /// Start scanning
    scan_start: Option<Instant>,
    /// Current scan pos: (out_idx, in_idx)
    scan_pos: (usize, usize),
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> RowPins<true> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, true>
{
    type RowPinsType = [In; ROW];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> RowPins<false> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, false>
{
    type RowPinsType = [Out; ROW];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> ColPins<false> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, false>
{
    type ColPinsType = [In; COL];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> ColPins<true> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, true>
{
    type ColPinsType = [Out; COL];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> MatrixOutputPins<Out> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, true>
{
    fn get_output_pins(&self) -> &[Out] {
        &self.col_pins
    }

    fn get_output_pins_mut(&mut self) -> &mut [Out] {
        &mut self.col_pins
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> MatrixOutputPins<Out> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, false>
{
    fn get_output_pins(&self) -> &[Out] {
        &self.row_pins
    }

    fn get_output_pins_mut(&mut self) -> &mut [Out] {
        &mut self.row_pins
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> MatrixInputPins<In> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, true>
{
    fn get_input_pins(&self) -> &[In] {
        &self.row_pins
    }

    fn get_input_pins_mut(&mut self) -> &mut [In] {
        &mut self.row_pins
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_input_pins(&mut self) {
        let mut futs: Vec<_, ROW> = self
            .get_input_pins_mut()
            .iter_mut()
            .map(|input_pin| input_pin.wait_for_high())
            .collect();
        let _ = select_slice(pin!(futs.as_mut_slice())).await;
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
> MatrixInputPins<In> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, false>
{
    fn get_input_pins(&self) -> &[In] {
        &self.col_pins
    }

    fn get_input_pins_mut(&mut self) -> &mut [In] {
        &mut self.col_pins
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_input_pins(&mut self) {
        let mut futs: Vec<_, COL> = self
            .get_input_pins_mut()
            .iter_mut()
            .map(|input_pin| input_pin.wait_for_high())
            .collect();
        let _ = select_slice(pin!(futs.as_mut_slice())).await;
    }
}

impl<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> InputDevice for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, COL2ROW>
where
    Self: RowPins<COL2ROW>,
    Self: ColPins<COL2ROW>,
    Self: MatrixOutputPins<Out>,
    Self: MatrixInputPins<In>,
{
    async fn read_event(&mut self) -> Event {
        loop {
            let (out_idx_start, in_idx_start) = self.scan_pos;

            #[cfg(feature = "async_matrix")]
            self.wait_for_key().await;

            // Scan matrix and send report
            for out_idx in out_idx_start..Self::OUTPUT_PIN_NUM {
                // Pull up output pin, wait 1us ensuring the change comes into effect
                if let Some(out_pin) = self.get_output_pins_mut().get_mut(out_idx) {
                    out_pin.set_high().ok();
                }
                Timer::after_micros(1).await;
                for in_idx in in_idx_start..Self::INPUT_PIN_NUM {
                    let in_pin_state = if let Some(in_pin) = self.get_input_pins_mut().get_mut(in_idx) {
                        in_pin.is_high().ok().unwrap_or_default()
                    } else {
                        false
                    };
                    // Check input pins and debounce
                    let debounce_state = self.debouncer.detect_change_with_debounce(
                        in_idx,
                        out_idx,
                        in_pin_state,
                        &self.get_key_state(out_idx, in_idx),
                    );

                    if let DebounceState::Debounced = debounce_state {
                        self.toggle_key_state(out_idx, in_idx);
                        self.scan_pos = (out_idx, in_idx);
                        return Event::Key(self.get_key_event(out_idx, in_idx));
                    }

                    // If there's key still pressed, always refresh the self.scan_start
                    #[cfg(feature = "async_matrix")]
                    if self.get_key_state(out_idx, in_idx).pressed {
                        self.scan_start = Some(Instant::now());
                    }
                }
                // Pull it back to low
                if let Some(out_pin) = self.get_output_pins_mut().get_mut(out_idx) {
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
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> MatrixTrait<ROW, COL> for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, COL2ROW>
where
    Self: RowPins<COL2ROW>,
    Self: ColPins<COL2ROW>,
    Self: MatrixOutputPins<Out>,
    Self: MatrixInputPins<In>,
{
    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        if let Some(start_time) = self.scan_start {
            // If not key over 2 secs, wait for interupt in next loop
            if start_time.elapsed().as_secs() < 1 {
                return;
            } else {
                self.scan_start = None;
            }
        }
        // First, set all output pin to high
        for out in self.get_output_pins_mut().iter_mut() {
            out.set_high().ok();
        }
        Timer::after_micros(1).await;

        // Wait for key
        self.wait_input_pins().await;

        // Set all output pins back to low
        for out in self.get_output_pins_mut().iter_mut() {
            out.set_low().ok();
        }

        self.scan_start = Some(Instant::now());
    }
}

impl<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, ROW, COL, COL2ROW>
where
    Self: RowPins<COL2ROW>,
    Self: ColPins<COL2ROW>,
{
    const OUTPUT_PIN_NUM: usize = const { if COL2ROW { COL } else { ROW } };
    const INPUT_PIN_NUM: usize = const { if COL2ROW { ROW } else { COL } };

    /// Create a matrix from input and output pins.
    /// Initialization of central
    pub fn new(
        row_pins: <Self as RowPins<COL2ROW>>::RowPinsType,
        col_pins: <Self as ColPins<COL2ROW>>::ColPinsType,
        debouncer: D,
    ) -> Self {
        CentralMatrix {
            row_pins,
            col_pins,
            debouncer,
            key_states: [[KeyState::default(); ROW]; COL],
            scan_start: None,
            scan_pos: (0, 0),
        }
    }

    fn get_key_event(&self, out_idx: usize, in_idx: usize) -> KeyboardEvent {
        if COL2ROW {
            KeyboardEvent::key(
                (in_idx + ROW_OFFSET) as u8,
                (out_idx + COL_OFFSET) as u8,
                self.key_states[out_idx][in_idx].pressed,
            )
        } else {
            KeyboardEvent::key(
                (out_idx + ROW_OFFSET) as u8,
                (in_idx + COL_OFFSET) as u8,
                self.key_states[in_idx][out_idx].pressed,
            )
        }
    }

    fn get_key_state(&self, out_idx: usize, in_idx: usize) -> KeyState {
        if COL2ROW {
            self.key_states[out_idx][in_idx]
        } else {
            self.key_states[in_idx][out_idx]
        }
    }

    fn toggle_key_state(&mut self, out_idx: usize, in_idx: usize) {
        if COL2ROW {
            self.key_states[out_idx][in_idx].toggle_pressed();
        } else {
            self.key_states[in_idx][out_idx].toggle_pressed();
        }
    }
}

/// DirectPinMartex only has input pins.
pub struct CentralDirectPinMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    D: DebouncerTrait<ROW, COL>,
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
    D: DebouncerTrait<ROW, COL>,
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
    D: DebouncerTrait<ROW, COL>,
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

                        if let DebounceState::Debounced = debounce_state {
                            self.key_states[row_idx][col_idx].toggle_pressed();
                            let (col, row, key_state) = (
                                (col_idx + COL_OFFSET) as u8,
                                (row_idx + ROW_OFFSET) as u8,
                                self.key_states[row_idx][col_idx],
                            );

                            self.scan_pos = (row_idx, col_idx);
                            return Event::Key(KeyboardEvent::key(row, col, key_state.pressed));
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
    D: DebouncerTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const ROW: usize,
    const COL: usize,
    const SIZE: usize,
> MatrixTrait<ROW, COL> for CentralDirectPinMatrix<In, D, ROW_OFFSET, COL_OFFSET, ROW, COL, SIZE>
{
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
                for direct_pin in direct_pins_row.iter_mut().flatten() {
                    let _ = futs.push(direct_pin.wait_for_low());
                }
            }
            let _ = select_slice(pin!(futs.as_mut_slice())).await;
        } else {
            let mut futs: Vec<_, SIZE> = Vec::new();
            for direct_pins_row in self.direct_pins.iter_mut() {
                for direct_pin in direct_pins_row.iter_mut().flatten() {
                    let _ = futs.push(direct_pin.wait_for_high());
                }
            }
            let _ = select_slice(pin!(futs.as_mut_slice())).await;
        }
        self.scan_start = Some(Instant::now());
    }
}
