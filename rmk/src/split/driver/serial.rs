use embassy_futures::select::{select, Either};
use embedded_io_async::Read;
use postcard::experimental::max_size::MaxSize;

use crate::split::master::{KeySyncMessage, SplitMessage, MASTER_SYNC_CHANNELS};

/// SerialSplitMasterReceiver receives split message from corresponding slave via serial.
/// The `ROW` and `COL` are the number of rows and columns of the corresponding slave's keyboard matrix.
/// The `ROW_OFFSET` and `COL_OFFSET` are the offset of the slave's matrix in the keyboard's matrix.
pub(crate) struct SerialSplitMasterReceiver<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    R: Read,
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
        R: Read,
    > SerialSplitMasterReceiver<ROW, COL, ROW_OFFSET, COL_OFFSET, R>
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
        let mut buf = [0_u8; SplitMessage::POSTCARD_MAX_SIZE];
        loop {
            let receive_fut = self.receiver.read(&mut buf);
            let sync_fut = MASTER_SYNC_CHANNELS[self.id].receive();
            match select(receive_fut, sync_fut).await {
                Either::First(received_message) => {
                    if let Ok(n_bytes) = received_message {
                        if n_bytes == 0 {
                            continue;
                        }
                        let message: SplitMessage = postcard::from_bytes(&buf).unwrap();
                        // Update the key state matrix
                        if let SplitMessage::Key(row, col, pressed) = message {
                            self.pressed[row as usize][col as usize] = pressed;
                        }
                    }
                }
                Either::Second(sync_message) => {
                    if let KeySyncMessage::StartRead = sync_message {
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
                    }
                }
            }
        }
    }
}
