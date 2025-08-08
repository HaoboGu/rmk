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
#[cfg(feature = "matrix_tester")]
pub struct MatrixState<const ROW: usize, const COL: usize> {
    // 30 bytes is the limited by Vial and 240 keys is enough for
    // most keyborad
    state: [u8; 30],
}

#[cfg(feature = "matrix_tester")]
impl<const ROW: usize, const COL: usize> MatrixState<ROW, COL> {
    const ROW: u8 = ROW as u8;
    const COL: u8 = COL as u8;
    const ROW_LEN: u8 = (COL as u8 + 8) / 8;
    const OUT_OF_BOUNDARY: () = if Self::ROW * Self::ROW_LEN > 30 {
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
        if let KeyboardEventPos::Key(key) = event.pos {
            let row = key.row;
            let col = key.col;
            if row >= Self::ROW || col >= Self::COL {
                warn!("Matrix read out of bounds");
                return;
            }
            let pressed = event.pressed;
            let index = row * Self::ROW_LEN * 8 + col;
            let byte_index = index / 8;
            let bit_index = index % 8;
            self.state[byte_index as usize] =
                self.state[byte_index as usize] & !(1 << bit_index) | ((pressed as u8) << bit_index);
        }
    }
    pub fn read_all(&self, target: &mut [u8]) {
        let slice = &self.state[..(Self::ROW as usize * Self::ROW_LEN as usize)];
        let mut target_iter = target.iter_mut();
        for row_bytes in slice.chunks(Self::ROW_LEN as usize) {
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
        if row >= Self::ROW || col >= Self::COL {
            warn!("Matrix read out of bounds");
            return false;
        }
        let index = row * Self::ROW_LEN * 8 + col;
        let byte_index = index / 8;
        let bit_index = index % 8;
        self.state[byte_index as usize] & (1 << bit_index) != 0
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
    /// Current scan pos: (out_idx, in_idx)
    scan_pos: (usize, usize),
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
    pub fn new(input_pins: [In; INPUT_PIN_NUM], output_pins: [Out; OUTPUT_PIN_NUM], debouncer: D) -> Self {
        Matrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: [[KeyState::new(); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            scan_start: None,
            scan_pos: (0, 0),
        }
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
> InputDevice for Matrix<In, Out, D, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    async fn read_event(&mut self) -> crate::event::Event {
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

                    if let DebounceState::Debounced = debounce_state {
                        self.key_states[out_idx][in_idx].toggle_pressed();
                        #[cfg(feature = "col2row")]
                        let (row, col, key_state) = (in_idx, out_idx, self.key_states[out_idx][in_idx]);
                        #[cfg(not(feature = "col2row"))]
                        let (row, col, key_state) = (out_idx, in_idx, self.key_states[out_idx][in_idx]);

                        self.scan_pos = (out_idx, in_idx);
                        return Event::Key(KeyboardEvent::key(row as u8, col as u8, key_state.pressed));
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
        }
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
> MatrixTrait for Matrix<In, Out, D, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
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
    fn wait_for_key(&mut self) -> impl Future<Output = ()> {
        async {}
    }
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
