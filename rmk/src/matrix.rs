use core::future::Future;
use core::sync::atomic::Ordering;

use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

use crate::CONNECTION_STATE;
use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::{Event, KeyboardEvent};
use crate::input_device::InputDevice;
use crate::state::ConnectionState;

/// Recording the matrix pressed state
#[cfg(feature = "vial_lock")]
pub struct MatrixState<const ROW: usize, const COL: usize> {
    // 30 bytes is the limited by Vial and 240 keys is enough for
    // most keyborad
    state: [u8; 30],
}

#[cfg(feature = "vial_lock")]
impl<const ROW: usize, const COL: usize> Default for MatrixState<ROW, COL> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "vial_lock")]
impl<const ROW: usize, const COL: usize> MatrixState<ROW, COL> {
    const ROW_LEN: usize = (COL + 8) / 8;
    const OUT_OF_BOUNDARY: () = if ROW * Self::ROW_LEN > 30 {
        panic!(
            "Cannot use matrix tester because your keyboard has too many keys. \
            Consider disable the `matrix_tester` feature"
        )
    };
    pub fn new() -> Self {
        Self { state: [0; 30] }
    }
    pub fn update(&mut self, event: &crate::event::KeyboardEvent) {
        use crate::event::KeyboardEventPos;
        if let KeyboardEventPos::Key(crate::event::KeyPos { row, col }) = event.pos {
            if row as usize >= ROW || col as usize >= COL {
                warn!("Matrix read out of bounds");
                return;
            }
            let pressed = event.pressed;
            let index = row as usize * Self::ROW_LEN * 8 + col as usize;
            let byte_index = index / 8;
            let bit_index = index % 8;
            self.state[byte_index] = self.state[byte_index] & !(1 << bit_index) | ((pressed as u8) << bit_index);
        }
    }
    pub fn read_all(&self, target: &mut [u8]) {
        let slice = &self.state[..(ROW * Self::ROW_LEN)];
        let mut target_iter = target.iter_mut();
        for row_bytes in slice.chunks(Self::ROW_LEN) {
            for byte in row_bytes.iter().rev() {
                if let Some(target_byte) = target_iter.next() {
                    *target_byte = *byte;
                } else {
                    break;
                }
            }
        }
    }
    pub fn read(&self, row: u8, col: u8) -> bool {
        if row as usize >= ROW || col as usize >= COL {
            warn!("Matrix read out of bounds");
            return false;
        }
        let index = row as usize * Self::ROW_LEN * 8 + col as usize;
        let byte_index = index / 8;
        let bit_index = index % 8;
        self.state[byte_index] & (1 << bit_index) != 0
    }
}

/// MatrixTrait is the trait for keyboard matrix.
///
/// The keyboard matrix is a 2D matrix of keys, the matrix does the scanning and saves the result to each key's `KeyState`.
/// The `KeyState` at position (row, col) can be read by `get_key_state` and updated by `update_key_state`.
pub trait MatrixTrait: InputDevice {
    // Matrix size
    const ROW: usize;
    const COL: usize;

    // Wait for USB or BLE really connected
    fn wait_for_connected(&self) -> impl Future<Output = ()> {
        async {
            while CONNECTION_STATE.load(Ordering::Acquire) == Into::<bool>::into(ConnectionState::Disconnected) {
                embassy_time::Timer::after_millis(100).await;
            }
            info!("Connected, start scanning matrix");
        }
    }

    #[cfg(feature = "async_matrix")]
    fn wait_for_key(&mut self) -> impl Future<Output = ()>;
}

/// KeyState represents the state of a key.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyState {
    // True if the key is pressed
    pub pressed: bool,
    // True if the key's state is just changed
    // pub changed: bool,
}

impl Default for KeyState {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyState {
    pub fn new() -> Self {
        KeyState { pressed: false }
    }

    pub fn toggle_pressed(&mut self) {
        self.pressed = !self.pressed;
    }

    pub fn is_releasing(&self) -> bool {
        !self.pressed
    }

    pub fn is_pressing(&self) -> bool {
        self.pressed
    }
}

pub trait RowPins<const COL2ROW: bool> {
    type RowPinsType;
}
pub trait ColPins<const COL2ROW: bool> {
    type ColPinsType;
}

pub trait MatrixOutputPins<Out: OutputPin> {
    fn get_output_pins(&self) -> &[Out];
    fn get_output_pins_mut(&mut self) -> &mut [Out];
}

pub trait MatrixInputPins<In: InputPin> {
    fn get_input_pins(&self) -> &[In];
    fn get_input_pins_mut(&mut self) -> &mut [In];
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct Matrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
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
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> RowPins<true> for Matrix<In, Out, D, ROW, COL, true>
{
    type RowPinsType = [In; ROW];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> RowPins<false> for Matrix<In, Out, D, ROW, COL, false>
{
    type RowPinsType = [Out; ROW];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> ColPins<false> for Matrix<In, Out, D, ROW, COL, false>
{
    type ColPinsType = [In; COL];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> ColPins<true> for Matrix<In, Out, D, ROW, COL, true>
{
    type ColPinsType = [Out; COL];
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> MatrixOutputPins<Out> for Matrix<In, Out, D, ROW, COL, true>
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
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> MatrixOutputPins<Out> for Matrix<In, Out, D, ROW, COL, false>
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
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> MatrixInputPins<In> for Matrix<In, Out, D, ROW, COL, true>
{
    fn get_input_pins(&self) -> &[In] {
        &self.row_pins
    }

    fn get_input_pins_mut(&mut self) -> &mut [In] {
        &mut self.row_pins
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
> MatrixInputPins<In> for Matrix<In, Out, D, ROW, COL, false>
{
    fn get_input_pins(&self) -> &[In] {
        &self.col_pins
    }

    fn get_input_pins_mut(&mut self) -> &mut [In] {
        &mut self.col_pins
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> Matrix<In, Out, D, ROW, COL, COL2ROW>
where
    Self: RowPins<COL2ROW>,
    Self: ColPins<COL2ROW>,
    Self: MatrixOutputPins<Out>,
{
    const OUTPUT_PIN_NUM: usize = const { if COL2ROW { COL } else { ROW } };
    const INPUT_PIN_NUM: usize = const { if COL2ROW { ROW } else { COL } };

    /// Create a matrix from input and output pins.
    pub fn new(
        row_pins: <Self as RowPins<COL2ROW>>::RowPinsType,
        col_pins: <Self as ColPins<COL2ROW>>::ColPinsType,
        debouncer: D,
    ) -> Self {
        Matrix {
            row_pins,
            col_pins,
            debouncer,
            key_states: [[KeyState::new(); ROW]; COL],
            scan_start: None,
            scan_pos: (0, 0),
        }
    }

    fn get_key_event(&self, out_idx: usize, in_idx: usize) -> KeyboardEvent {
        if COL2ROW {
            KeyboardEvent::key(in_idx as u8, out_idx as u8, self.key_states[out_idx][in_idx].pressed)
        } else {
            KeyboardEvent::key(out_idx as u8, in_idx as u8, self.key_states[in_idx][out_idx].pressed)
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

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> InputDevice for Matrix<In, Out, D, ROW, COL, COL2ROW>
where
    Self: RowPins<COL2ROW>,
    Self: ColPins<COL2ROW>,
    Self: MatrixOutputPins<Out>,
    Self: MatrixInputPins<In>,
{
    async fn read_event(&mut self) -> crate::event::Event {
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
        }
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> MatrixTrait for Matrix<In, Out, D, ROW, COL, COL2ROW>
where
    Self: RowPins<COL2ROW>,
    Self: ColPins<COL2ROW>,
    Self: MatrixOutputPins<Out>,
    Self: MatrixInputPins<In>,
{
    const ROW: usize = ROW;
    const COL: usize = COL;

    #[cfg(feature = "async_matrix")]
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
        for out in self.get_output_pins_mut().iter_mut() {
            out.set_high().ok();
        }
        Timer::after_micros(1).await;
        {
            let mut futs: Vec<_, ROW> = self
                .get_input_pins_mut()
                .iter_mut()
                .map(|input_pin| input_pin.wait_for_high())
                .collect();
            let _ = select_slice(pin!(futs.as_mut_slice())).await;
        }

        // Set all output pins back to low
        for out in self.get_output_pins_mut().iter_mut() {
            out.set_low().ok();
        }

        self.scan_start = Some(Instant::now());
    }
}

pub struct TestMatrix<const ROW: usize, const COL: usize> {
    last: bool,
}
impl<const ROW: usize, const COL: usize> Default for TestMatrix<ROW, COL> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const ROW: usize, const COL: usize> TestMatrix<ROW, COL> {
    pub fn new() -> Self {
        Self { last: false }
    }
}
impl<const ROW: usize, const COL: usize> MatrixTrait for TestMatrix<ROW, COL> {
    const ROW: usize = ROW;
    const COL: usize = COL;

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {}
}

impl<const ROW: usize, const COL: usize> InputDevice for TestMatrix<ROW, COL> {
    async fn read_event(&mut self) -> Event {
        if self.last {
            embassy_time::Timer::after_millis(100).await;
        } else {
            embassy_time::Timer::after_secs(5).await;
        }
        self.last = !self.last;
        // info!("Read event: {:?}", self.last);
        Event::Key(KeyboardEvent::key(0, 0, self.last))
    }
}
