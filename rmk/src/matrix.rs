use crate::{
    debounce::{DebounceState, DebouncerTrait},
    event::KeyEvent,
    keyboard::KEY_EVENT_CHANNEL,
    CONNECTION_STATE,
};
use core::future::Future;
use defmt::{info, Format};
use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

/// MatrixTrait is the trait for keyboard matrix.
///
/// The keyboard matrix is a 2D matrix of keys, the matrix does the scanning and saves the result to each key's `KeyState`.
/// The `KeyState` at position (row, col) can be read by `get_key_state` and updated by `update_key_state`.
pub trait MatrixTrait {
    // Matrix size
    const ROW: usize;
    const COL: usize;

    // Wait for USB or BLE really connected
    fn wait_for_connected(&self) -> impl Future<Output = ()> {
        async {
            while !CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                embassy_time::Timer::after_millis(100).await;
            }
            info!("Connected, start scanning matrix");
        }
    }

    // Run the matrix
    fn run(&mut self) -> impl Future<Output = ()> {
        async {
            // We don't check disconnected state because disconnection means the task will be dropped
            loop {
                self.wait_for_connected().await;
                self.scan().await;
            }
        }
    }

    // Do matrix scanning, save the result in matrix's key_state field.
    fn scan(&mut self) -> impl Future<Output = ()>;

    // Read key state at position (row, col)
    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState;

    // Update key state at position (row, col)
    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState));

    // Get matrix row num
    fn get_row_num(&self) -> usize {
        Self::ROW
    }

    // Get matrix col num
    fn get_col_num(&self) -> usize {
        Self::COL
    }

    #[cfg(feature = "async_matrix")]
    fn wait_for_key(&mut self) -> impl Future<Output = ()>;
}

/// KeyState represents the state of a key.
#[derive(Copy, Clone, Debug, Format)]
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
    pub fn new(
        input_pins: [In; INPUT_PIN_NUM],
        output_pins: [Out; OUTPUT_PIN_NUM],
        debouncer: D,
    ) -> Self {
        Matrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: [[KeyState::new(); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            scan_start: None,
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
            // KEY_EVENT_CHANNEL
            //     .send(KeyEvent {
            //         row: 0,
            //         col: 0,
            //         pressed: true,
            //     })
            //     .await;
            // embassy_time::Timer::after_micros(200).await;
            // KEY_EVENT_CHANNEL
            //     .send(KeyEvent {
            //         row: 0,
            //         col: 0,
            //         pressed: false,
            //     })
            //     .await;
            // embassy_time::Timer::after_secs(5).await;
            // #[cfg(feature = "async_matrix")]
            // self.wait_for_key().await;

            // // Scan matrix and send report
            // for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
            //     // Pull up output pin, wait 1us ensuring the change comes into effect
            //     out_pin.set_high().ok();
            //     Timer::after_micros(1).await;
            //     for (in_idx, in_pin) in self.input_pins.iter_mut().enumerate() {
            //         // Check input pins and debounce
            //         let debounce_state = self.debouncer.detect_change_with_debounce(
            //             in_idx,
            //             out_idx,
            //             in_pin.is_high().ok().unwrap_or_default(),
            //             &self.key_states[out_idx][in_idx],
            //         );

            //         match debounce_state {
            //             DebounceState::Debounced => {
            //                 self.key_states[out_idx][in_idx].toggle_pressed();
            //                 #[cfg(feature = "col2row")]
            //                 let (row, col, key_state) =
            //                     (in_idx, out_idx, self.key_states[out_idx][in_idx]);
            //                 #[cfg(not(feature = "col2row"))]
            //                 let (row, col, key_state) =
            //                     (out_idx, in_idx, self.key_states[out_idx][in_idx]);

            //                 KEY_EVENT_CHANNEL
            //                     .send(KeyEvent {
            //                         row: row as u8,
            //                         col: col as u8,
            //                         pressed: key_state.pressed,
            //                     })
            //                     .await;
            //             }
            //             _ => (),
            //         }

            //         // If there's key still pressed, always refresh the self.scan_start
            //         #[cfg(feature = "async_matrix")]
            //         if self.key_states[out_idx][in_idx].pressed {
            //             self.scan_start = Some(Instant::now());
            //         }
            //     }
            //     out_pin.set_low().ok();
            // }

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
