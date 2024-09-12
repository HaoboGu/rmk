///! The abstracted driver layer of the split keyboard.
///!
///!
use crate::split::SYNC_SIGNALS;
#[cfg(feature = "async_matrix")]
use crate::{
    split::{KeySyncSignal, SCAN_SIGNAL},
    KEYBOARD_STATE,
};

use super::{KeySyncMessage, SplitMessage, CENTRAL_SYNC_CHANNELS};
use defmt::debug;
use embassy_futures::select::{select, Either};

#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum SplitDriverError {
    SerialError,
    EmptyMessage,
    DeserializeError,
    SerializeError,
    BleError(u8),
}

/// Split message reader from other split devices
pub(crate) trait SplitReader {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError>;
}

/// Split message writer to other split devices
pub(crate) trait SplitWriter {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError>;
}

/// PeripheralMatrixMonitor runs in central.
/// It reads split message from peripheral and updates key matrix cache of the peripheral.
///
/// When the central scans the matrix, the scanning thread sends sync signal and gets key state cache back.
///
/// The `ROW` and `COL` are the number of rows and columns of the corresponding peripheral's keyboard matrix.
/// The `ROW_OFFSET` and `COL_OFFSET` are the offset of the peripheral's matrix in the keyboard's matrix.
pub(crate) struct PeripheralMatrixMonitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    R: SplitReader,
> {
    /// Key state cache matrix
    pressed: [[bool; COL]; ROW],
    /// Receiver
    receiver: R,
    /// Peripheral id
    id: usize,
}

impl<
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        R: SplitReader,
    > PeripheralMatrixMonitor<ROW, COL, ROW_OFFSET, COL_OFFSET, R>
{
    pub(crate) fn new(receiver: R, id: usize) -> Self {
        Self {
            pressed: [[false; COL]; ROW],
            receiver,
            id,
        }
    }

    /// Run the monitor.
    ///
    /// The monitor receives from the peripheral and waits for the sync message from the matrix scanning thread.
    /// If a sync message is received from matrix scanning thread, it sends the key state matrix back.
    pub(crate) async fn run(mut self) -> ! {
        loop {
            let receive_fut = self.receiver.read();
            let sync_fut = SYNC_SIGNALS[self.id].wait();
            match select(receive_fut, sync_fut).await {
                Either::First(received_message) => {
                    debug!("Receveid peripheral message: {}", received_message);
                    if let Ok(message) = received_message {
                        // Update the key state matrix
                        if let SplitMessage::Key(row, col, pressed) = message {
                            self.pressed[row as usize][col as usize] = pressed;
                        }
                        // In async matrix mode, signal to start matrix scanning
                        #[cfg(feature = "async_matrix")]
                        if KEYBOARD_STATE.load(core::sync::atomic::Ordering::Relaxed) {
                            SCAN_SIGNAL.signal(KeySyncSignal::Start);
                        }
                    }
                }
                Either::Second(_sync_signal) => {
                    // Start synchronizing key state matrix

                    // First, send the number of states to be sent
                    CENTRAL_SYNC_CHANNELS[self.id]
                        .send(KeySyncMessage::StartSend((ROW * COL) as u16))
                        .await;
                    // Send the key state matrix
                    // TODO: Optimize: send only the changed key states
                    for row in 0..ROW {
                        for col in 0..COL {
                            CENTRAL_SYNC_CHANNELS[self.id]
                                .send(KeySyncMessage::Key(
                                    (row + ROW_OFFSET) as u8,
                                    (col + COL_OFFSET) as u8,
                                    self.pressed[row][col],
                                ))
                                .await;
                        }
                    }

                    // Reset key signal, enable next scan
                    SYNC_SIGNALS[self.id].reset();
                }
            }
        }
    }
}
