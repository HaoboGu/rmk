use core::sync::atomic::Ordering;

///! The abstracted driver layer of the split keyboard.
///!
use super::SplitMessage;
use crate::CONNECTION_STATE;
use crate::{event::KeyEvent, keyboard::KEY_EVENT_CHANNEL};
use defmt::{debug, error, warn};
use embassy_futures::select::select;
use heapless::Vec;

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
    async fn read(&mut self) -> Result<Vec<SplitMessage, 2>, SplitDriverError>;
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
/// TODO: Rename `PeripheralMatrixMonitor`
pub(crate) struct PeripheralMatrixMonitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    R: SplitReader + SplitWriter,
> {
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
        R: SplitReader + SplitWriter,
    > PeripheralMatrixMonitor<ROW, COL, ROW_OFFSET, COL_OFFSET, R>
{
    pub(crate) fn new(receiver: R, id: usize) -> Self {
        Self { receiver, id }
    }

    /// Run the monitor.
    ///
    /// The monitor receives from the peripheral and forward the message to `KEY_EVENT_CHANNEL`.
    pub(crate) async fn run(mut self) -> ! {
        let mut conn_state = CONNECTION_STATE.load(Ordering::Acquire);
        // Send once on start
        if let Err(e) = self
            .receiver
            .write(&SplitMessage::ConnectionState(conn_state))
            .await
        {
            error!("SplitDriver write error: {}", e);
        }
        loop {
            defmt::info!("test");
            match select(self.receiver.read(), embassy_time::Timer::after_millis(500)).await {
                embassy_futures::select::Either::First(read_result) => match read_result {
                    Ok(received_messages) => {
                        for received_message in received_messages {
                            debug!("Received peripheral message: {}", received_message);
                            if let SplitMessage::Key(e) = received_message {
                                // Check row/col
                                if e.row as usize > ROW || e.col as usize > COL {
                                    error!("Invalid peripheral row/col: {} {}", e.row, e.col);
                                    continue;
                                }

                                if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                                    // Only when the connection is established, send the key event.
                                    KEY_EVENT_CHANNEL
                                        .send(KeyEvent {
                                            row: e.row + ROW_OFFSET as u8,
                                            col: e.col + COL_OFFSET as u8,
                                            pressed: e.pressed,
                                        })
                                        .await;
                                }
                            } else {
                                warn!("Key event from peripheral is ignored because the connection is not established.");
                            }
                        }
                    }
                    Err(e) => error!("Peripheral message read error: {:?}", e),
                },
                embassy_futures::select::Either::Second(_) => {
                    // Sync ConnectionState every 500ms
                    conn_state = CONNECTION_STATE.load(Ordering::Acquire);
                    if let Err(e) = self
                        .receiver
                        .write(&SplitMessage::ConnectionState(conn_state))
                        .await
                    {
                        error!("SplitDriver write error: {}", e);
                    };
                }
            }
        }
    }
}
