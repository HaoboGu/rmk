use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::{
    debounce::{DebounceState, DebouncerTrait},
    matrix::{KeyState, MatrixTrait},
};

/// Channels for synchronization between master and slave threads
const SYNC_CHANNEL_VALUE: Channel<CriticalSectionRawMutex, SyncMessage, 8> = Channel::new();
pub(crate) static MASTER_SYNC_CHANNELS: [Channel<CriticalSectionRawMutex, SyncMessage, 8>; 4] =
    [SYNC_CHANNEL_VALUE; 4];

#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize)]
#[repr(u8)]
pub enum SplitMessage {
    /// Activated key info (row, col, pressed), from slave to master
    Key(u8, u8, bool),
    /// Led state, on/off
    LedState(bool),
}

/// Message used for synchronization between master thread and slave thread
pub(crate) enum SyncMessage {
    /// Sent from master to slave thread, indicating master starts to read the key state matrix
    StartRead,
    /// Response of `StartRead`, sent from slave to master, indicating that the slave starts to send the key state matrix.
    /// u8 is the number of sent key states
    StartSend(u16),
    /// Key state: (row, col, key_pressing_state)
    Key(u8, u8, bool),
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub struct MasterMatrix<
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
> {
    /// Input pins of the pcb matrix
    input_pins: [In; INPUT_PIN_NUM],
    /// Output pins of the pcb matrix
    output_pins: [Out; OUTPUT_PIN_NUM],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; COL]; ROW],
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
    async fn scan(&mut self) {
        self.internal_scan().await;
        self.scan_slave().await;
    }

    fn get_key_state(&mut self, _row: usize, _col: usize) -> KeyState {
        todo!()
    }

    fn update_key_state(&mut self, _row: usize, _col: usize, _f: impl FnOnce(&mut KeyState)) {
        todo!()
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
    /// Initialization of master:
    ///
    /// 1. the matrix definition of master board: (row, col), (total_row, total_col), (row_pins, col_pins)
    /// 2. keyboard definition
    /// 3. storage definition
    pub(crate) fn new(_input_pins: [In; INPUT_PIN_NUM], _output_pins: [Out; OUTPUT_PIN_NUM]) -> Self {
        todo!()
    }

    pub(crate) async fn scan_slave(&mut self) {
        for slave_channel in MASTER_SYNC_CHANNELS.iter() {
            slave_channel.send(SyncMessage::StartRead).await;
            if let SyncMessage::StartSend(n) = slave_channel.receive().await {
                for _ in 0..n {
                    if let SyncMessage::Key(row, col, key_state) = slave_channel.receive().await {
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

pub(crate) struct SlaveCache<const ROW: usize, const COL: usize /* , R: Read*/> {
    /// Key state matrix
    pressed: [[bool; COL]; ROW],
    /// Receiver
    // receiver: R,
    /// Slave id
    id: usize,
}

impl<const ROW: usize, const COL: usize /* , R: Read*/> SlaveCache<ROW, COL> {
    pub(crate) fn new(id: usize) -> Self {
        Self {
            pressed: [[false; COL]; ROW],
            // receiver: todo!(),
            id,
        }
    }
    // pub(crate) fn new(receiver: R, id: usize) -> Self {
    //     Self {
    //         pressed: [[false; COL]; ROW],
    //         receiver,
    //         id,
    //     }
    // }

    pub(crate) async fn run(self) -> ! {
        let mut _buf = [0_u8; SplitMessage::POSTCARD_MAX_SIZE];
        loop {
            // let receive_from_slave = self.receiver.read(&mut buf);
            // let slave_sync = MASTER_SYNC_CHANNELS[self.id].receive();
            // match select(receive_from_slave, slave_sync).await {
            //     Either::First(receive_re) => {
            //         if let Ok(n_bytes) = receive_re {
            //             if n_bytes == 0 {
            //                 continue;
            //             }
            //             let message: SplitMessage = postcard::from_bytes(&buf).unwrap();
            //             // Update the key state matrix
            //             if let SplitMessage::Key(row, col, pressed) = message {
            //                 // TODO: Check row, col
            //                 self.pressed[row as usize][col as usize] = pressed;
            //             }
            //         }
            //     }
            //     Either::Second(sync_message) => {
            //         if let SyncMessage::StartRead = sync_message {
            //             // First, send the number of states to be sent
            //             MASTER_SYNC_CHANNELS[self.id]
            //                 .send(SyncMessage::StartSend((ROW * COL) as u16))
            //                 .await;

            //             // Send the key state matrix
            //             // TODO: Check index with slave offset
            //             for i in 0..ROW {
            //                 for j in 0..COL {
            //                     MASTER_SYNC_CHANNELS[self.id]
            //                         .send(SyncMessage::Key(i as u8, j as u8, self.pressed[i][j]))
            //                         .await;
            //                 }
            //             }
            //         }
            //     }
            // }
        }
    }
}
