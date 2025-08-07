use core::future::Future;
#[cfg(feature = "matrix_tester")]
use core::sync::atomic::AtomicU8;
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

/// Recording the matrix state
#[cfg(feature = "matrix_tester")]
pub static MATRIX_STATE: MatrixState = MatrixState::new();

/// Recording the matrix pressed state
#[cfg(feature = "matrix_tester")]
pub struct MatrixState {
    /// the maxima available is 30 bytes due to the VIA `report.input_data` is `[u8;32]`.
    state: bitvec::BitArr!(for 240, in AtomicU8),
    row: AtomicU8,
    row_len: AtomicU8,
}

#[cfg(feature = "matrix_tester")]
impl MatrixState {
    const fn new() -> Self {
        Self {
            state: bitvec::prelude::BitArray::ZERO,
            row: AtomicU8::new(0),
            row_len: AtomicU8::new(0),
        }
    }
    pub fn update(&self, event: &crate::event::KeyboardEvent) {
        use crate::event::KeyboardEventPos;
        if let KeyboardEventPos::Key(key) = event.pos {
            let row = key.row;
            let col = key.col;
            let pressed = event.pressed;
            let index = row * self.row_len.load(Ordering::Relaxed) * 8 + col;
            // The bitvec crate does not mainly design for atomic operations.
            // And the safe interface does not provide a modification method
            // but we know that it is safe because the underlying data is atomic.
            unsafe { self.state.as_bitptr().offset(index as isize).to_mut().write(pressed) };
        }
    }
    pub fn configure(&self, row: usize, col: usize) {
        self.row.store(row as u8, Ordering::Relaxed);
        let row_len = bitvec::mem::elts::<u8>(col);
        self.row_len.store(row_len as u8, Ordering::Relaxed);
    }
    pub fn read_all(&self, target: &mut [u8]) {
        let row = self.row.load(Ordering::Relaxed);
        let row_len = self.row_len.load(Ordering::Relaxed);
        let slice = &self.state.as_raw_slice()[..(row as usize * row_len as usize)];
        let mut target_iter = target.iter_mut();
        for row_bytes in slice.chunks(row_len.into()) {
            for byte in row_bytes.iter().rev() {
                *target_iter.next().unwrap() = byte.load(Ordering::Relaxed);
            }
        }
    }
    pub fn read(&self, row: u8, col: u8) -> bool {
        let row_len = self.row_len.load(Ordering::Relaxed);
        *self
            .state
            .get(row as usize * row_len as usize * 8 + col as usize)
            .unwrap()
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
