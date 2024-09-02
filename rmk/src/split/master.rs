use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;

use crate::debounce::{DebounceState, DebouncerTrait};
use crate::matrix::{KeyState, MatrixTrait};
use crate::split::KeySyncSignal;
use crate::split::SYNC_SIGNALS;

use super::{KeySyncMessage, MASTER_SYNC_CHANNELS};

/// Matrix is the physical pcb layout of the keyboard matrix.
pub(crate) struct MasterMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
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
    key_states: [[KeyState; TOTAL_COL]; TOTAL_ROW],
    /// Start scanning
    scan_start: Option<Instant>,
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > MatrixTrait
    for MasterMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    const ROW: usize = ROW;
    const COL: usize = COL;

    async fn scan(&mut self) {
        self.internal_scan().await;
        self.scan_slave().await;
    }

    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        self.key_states[row][col]
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        f(&mut self.key_states[row][col]);
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        todo!()
    }
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > MasterMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Initialization of master
    pub(crate) fn new(
        input_pins: [In; INPUT_PIN_NUM],
        output_pins: [Out; OUTPUT_PIN_NUM],
        debouncer: D,
    ) -> Self {
        MasterMatrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: [[KeyState::default(); COL]; ROW],
            scan_start: None,
        }
    }

    pub(crate) async fn scan_slave(&mut self) {
        for (id, slave_channel) in MASTER_SYNC_CHANNELS.iter().enumerate() {
            // TODO: Skip unused slaves
            if id > 0 {
                break;
            }
            // Signal that slave scanning is started
            SYNC_SIGNALS[id].signal(KeySyncSignal::Start);
            // Receive slave key states
            if let KeySyncMessage::StartSend(n) = slave_channel.receive().await {
                // Update slave's key states
                for _ in 0..n {
                    if let KeySyncMessage::Key(row, col, key_state) = slave_channel.receive().await
                    {
                        if key_state != self.key_states[row as usize][col as usize].pressed {
                            self.key_states[row as usize][col as usize].pressed = key_state;
                            self.key_states[row as usize][col as usize].changed = true;
                        } else {
                            self.key_states[row as usize][col as usize].changed = false;
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn internal_scan(&mut self) {
        // Get the row and col index of current board in the whole key matrix
        for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
            // Pull up output pin, wait 1us ensuring the change comes into effect
            out_pin.set_high().ok();
            Timer::after_micros(1).await;
            for (in_idx, in_pin) in self.input_pins.iter_mut().enumerate() {
                #[cfg(feature = "col2row")]
                let (row_idx, col_idx) = (in_idx + ROW_OFFSET, out_idx + COL_OFFSET);
                #[cfg(not(feature = "col2row"))]
                let (row_idx, col_idx) = (out_idx + ROW_OFFSET, in_idx + COL_OFFSET);

                // Check input pins and debounce
                let debounce_state = self.debouncer.detect_change_with_debounce(
                    in_idx,
                    out_idx,
                    in_pin.is_high().ok().unwrap_or_default(),
                    &self.key_states[row_idx][col_idx],
                );

                match debounce_state {
                    DebounceState::Debounced => {
                        self.key_states[row_idx][col_idx].toggle_pressed();
                        self.key_states[row_idx][col_idx].changed = true;
                    }
                    _ => self.key_states[row_idx][col_idx].changed = false,
                }

                // If there's key changed or pressed, always refresh the self.scan_start
                if self.key_states[row_idx][col_idx].changed
                    || self.key_states[row_idx][col_idx].pressed
                {
                    #[cfg(feature = "async_matrix")]
                    {
                        self.scan_start = Some(Instant::now());
                    }
                }
            }
            out_pin.set_low().ok();
        }
    }

    /// Read key state OF CURRENT BOARD at position (row, col)
    pub(crate) fn get_key_state_current_board(
        &mut self,
        out_idx: usize,
        in_idx: usize,
    ) -> KeyState {
        #[cfg(feature = "col2row")]
        return self.key_states[in_idx + ROW_OFFSET][out_idx + COL_OFFSET];
        #[cfg(not(feature = "col2row"))]
        return self.key_states[out_idx + ROW_OFFSET][in_idx + COL_OFFSET];
    }
}
