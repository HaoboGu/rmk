use crate::split::SYNC_SIGNALS;

use super::{KeySyncMessage, SplitMessage, MASTER_SYNC_CHANNELS};
use defmt::info;
use embassy_futures::select::{select, Either};

#[cfg(feature = "_nrf_ble")]
pub(crate) mod nrf_ble;
pub(crate) mod serial;

#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum SplitDriverError {
    SerialError,
    EmptyMessage,
    DeserializeError,
    SerializeError,
    BleError(u8),
}

pub(crate) trait SplitReader {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError>;
}
pub(crate) trait SplitWriter {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError>;
}

/// SplitMasterReceiver receives split message from corresponding slave via serial.
/// The `ROW` and `COL` are the number of rows and columns of the corresponding slave's keyboard matrix.
/// The `ROW_OFFSET` and `COL_OFFSET` are the offset of the slave's matrix in the keyboard's matrix.
pub(crate) struct SplitMasterReceiver<
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
    /// Slave id
    id: usize,
}

impl<
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        R: SplitReader,
    > SplitMasterReceiver<ROW, COL, ROW_OFFSET, COL_OFFSET, R>
{
    pub(crate) fn new(receiver: R, id: usize) -> Self {
        Self {
            pressed: [[false; COL]; ROW],
            receiver,
            id,
        }
    }

    /// Run the receiver.
    /// The receiver receives from the slave and waits for the sync message from the master matrix.
    /// If a sync message is received from master, it sends the key state matrix to the master matrix.
    pub(crate) async fn run(mut self) -> ! {
        loop {
            let receive_fut = self.receiver.read();
            let sync_fut = SYNC_SIGNALS[self.id].wait();
            match select(receive_fut, sync_fut).await {
                Either::First(received_message) => {
                    info!("Receveid slave message: {}", received_message);
                    if let Ok(message) = received_message {
                        // Update the key state matrix
                        if let SplitMessage::Key(row, col, pressed) = message {
                            self.pressed[row as usize][col as usize] = pressed;
                        }
                    }
                }
                Either::Second(_sync_signal) => {
                    // First, send the number of states to be sent
                    MASTER_SYNC_CHANNELS[self.id]
                        .send(KeySyncMessage::StartSend((ROW * COL) as u16))
                        .await;
                    // Send the key state matrix
                    // TODO: Optimize: send only the changed key states
                    for row in 0..ROW {
                        for col in 0..COL {
                            MASTER_SYNC_CHANNELS[self.id]
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
