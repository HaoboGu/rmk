#[cfg(feature = "async_matrix")]
use core::pin::pin;
use core::sync::atomic::Ordering;

use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

use crate::CONNECTION_STATE;
use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::{Event, KeyPos, KeyboardEvent, KeyboardEventPos};
use crate::input_device::InputDevice;
use crate::state::ConnectionState;

pub mod bidirectional_matrix;

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
pub trait MatrixTrait<const ROW: usize, const COL: usize>: InputDevice {
    // Wait for USB or BLE really connected
    async fn wait_for_connected(&self) {
        while CONNECTION_STATE.load(Ordering::Acquire) == Into::<bool>::into(ConnectionState::Disconnected) {
            embassy_time::Timer::after_millis(100).await;
        }
        info!("Connected, start scanning matrix");
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self);
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
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct Matrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> {
    /// Row pins of the pcb matrix
    row_pins: [In; ROW],
    /// Column pins of the pcb matrix
    col_pins: [Out; COL],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; ROW]; COL],
    /// Current scan pos: (out_idx, in_idx)
    scan_pos: (usize, usize),
    /// Re-scan needed flag
    #[cfg(feature = "async_matrix")]
    rescan_needed: bool,
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> Matrix<In, Out, D, ROW, COL, COL2ROW>
{
    /// Create a matrix from input and output pins.
    pub fn new(row_pins: [In; ROW], col_pins: [Out; COL], debouncer: D) -> Self {
        Matrix {
            row_pins,
            col_pins,
            debouncer,
            key_states: [[KeyState::new(); ROW]; COL],
            scan_pos: (0, 0),
            #[cfg(feature = "async_matrix")]
            rescan_needed: false,
        }
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_input_pins(&mut self) {
        let mut futs: Vec<_, ROW> = if COL2ROW {
            self.row_pins
                .iter_mut()
                .map(|input_pin| input_pin.wait_for_high())
                .collect()
        } else {
            self.row_pins
                .iter_mut()
                .map(|input_pin| input_pin.wait_for_high())
                .collect()
        };
        let _ = select_slice(pin!(futs.as_mut_slice())).await;
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> InputDevice for Matrix<In, Out, D, ROW, COL, COL2ROW>
{
    async fn read_event(&mut self) -> crate::event::Event {
        loop {
            let (col_idx_start, row_idx_start) = self.scan_pos;

            // Scan matrix and send report
            for col_idx in col_idx_start..COL {
                // Activate output pin, wait 1us ensuring the change comes into effect
                let out_pin = &mut self.col_pins[col_idx];
                if COL2ROW { out_pin.set_high() } else { out_pin.set_low() }.ok();

                // This may take >1ms on some platforms if other tasks are running!
                Timer::after_micros(1).await;

                let start = if col_idx == col_idx_start { row_idx_start } else { 0 };

                for row_idx in start..ROW {
                    let in_pin = &mut self.row_pins[row_idx];
                    let in_pin_state = if COL2ROW { in_pin.is_high() } else { in_pin.is_low() }
                        .ok()
                        .unwrap_or_default();

                    let state = &mut self.key_states[col_idx][row_idx];

                    // Check input pins and debounce
                    let debounce_state =
                        self.debouncer
                            .detect_change_with_debounce(row_idx, col_idx, in_pin_state, &state);

                    if let DebounceState::Debounced = debounce_state {
                        state.toggle_pressed();
                        self.scan_pos = (col_idx, row_idx);
                        #[cfg(feature = "async_matrix")]
                        {
                            self.rescan_needed = true;
                        }
                        return Event::Key(KeyboardEvent::key(row_idx as u8, col_idx as u8, state.pressed));
                    }

                    // If there's key still pressed, always refresh the self.scan_start
                    #[cfg(feature = "async_matrix")]
                    if state.pressed {
                        self.rescan_needed = true;
                    }
                }

                // deactivate output pin
                let out_pin = &mut self.col_pins[col_idx];
                if COL2ROW { out_pin.set_low() } else { out_pin.set_high() }.ok();
            }

            #[cfg(feature = "async_matrix")]
            {
                if !self.rescan_needed {
                    self.wait_for_key().await;
                }
                self.rescan_needed = false;
            }
            self.scan_pos = (0, 0);
        }
    }
}

impl<
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    Out: OutputPin,
    D: DebouncerTrait<ROW, COL>,
    const ROW: usize,
    const COL: usize,
    const COL2ROW: bool,
> MatrixTrait<ROW, COL> for Matrix<In, Out, D, ROW, COL, COL2ROW>
{
    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        // First, activate all output pins
        for out in self.col_pins.iter_mut() {
            if COL2ROW {
                out.set_high().ok();
            } else {
                out.set_low().ok();
            }
        }

        // Wait for any key press (any input pin activation)
        self.wait_input_pins().await;

        // deactivate all output pins
        for out in self.col_pins.iter_mut() {
            if COL2ROW {
                out.set_low().ok();
            } else {
                out.set_high().ok();
            }
        }
    }
}

pub struct OffsetMatrixWrapper<
    const ROW: usize,
    const COL: usize,
    M: MatrixTrait<ROW, COL>,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
>(pub M);

impl<const ROW: usize, const COL: usize, M: MatrixTrait<ROW, COL>, const ROW_OFFSET: usize, const COL_OFFSET: usize>
    InputDevice for OffsetMatrixWrapper<ROW, COL, M, ROW_OFFSET, COL_OFFSET>
{
    async fn read_event(&mut self) -> Event {
        match self.0.read_event().await {
            Event::Key(KeyboardEvent {
                pressed,
                pos: KeyboardEventPos::Key(KeyPos { row, col }),
            }) => Event::Key(KeyboardEvent::key(
                row + ROW_OFFSET as u8,
                col + COL_OFFSET as u8,
                pressed,
            )),
            event => event,
        }
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

impl<const ROW: usize, const COL: usize> MatrixTrait<ROW, COL> for TestMatrix<ROW, COL> {
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
