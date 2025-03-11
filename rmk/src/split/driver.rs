//! The abstracted driver layer of the split keyboard.
//!
use super::SplitMessage;
use crate::channel::EVENT_CHANNEL;
use crate::CONNECTION_STATE;
use crate::{channel::KEY_EVENT_CHANNEL, event::KeyEvent};
use core::sync::atomic::Ordering;
use embassy_futures::select::select;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

/// PeripheralManager runs in central.
/// It reads split message from peripheral and updates key matrix cache of the peripheral.
///
/// When the central scans the matrix, the scanning thread sends sync signal and gets key state cache back.
///
/// The `ROW` and `COL` are the number of rows and columns of the corresponding peripheral's keyboard matrix.
/// The `ROW_OFFSET` and `COL_OFFSET` are the offset of the peripheral's matrix in the keyboard's matrix.
pub(crate) struct PeripheralManager<
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
    > PeripheralManager<ROW, COL, ROW_OFFSET, COL_OFFSET, R>
{
    pub(crate) fn new(receiver: R, id: usize) -> Self {
        Self { receiver, id }
    }

    /// Run the manager.
    ///
    /// The manager receives from the peripheral and forward the message to `KEY_EVENT_CHANNEL`.
    /// It also sync the `ConnectionState` to the peripheral periodically.
    pub(crate) async fn run(mut self) -> ! {
        let mut conn_state = CONNECTION_STATE.load(Ordering::Acquire);
        // Send connection state once on start
        if let Err(e) = self
            .receiver
            .write(&SplitMessage::ConnectionState(conn_state))
            .await
        {
            error!("SplitDriver write error: {:?}", e);
        }
        loop {
            // Read the message from peripheral, or sync the connection state every 500ms.
            match select(self.receiver.read(), embassy_time::Timer::after_millis(500)).await {
                embassy_futures::select::Either::First(read_result) => match read_result {
                    Ok(received_message) => {
                        debug!("Received peripheral message: {:?}", received_message);
                        match received_message {
                            SplitMessage::Key(e) => {
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
                                } else {
                                    warn!("Key event from peripheral is ignored because the connection is not established.");
                                }
                            }
                            SplitMessage::Event(e) => {
                                if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                                    // Only when the connection is established, send the event
                                    EVENT_CHANNEL.send(e).await;
                                } else {
                                    warn!("Event from peripheral is ignored because the connection is not established.");
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => error!("Peripheral message read error: {:?}", e),
                },
                embassy_futures::select::Either::Second(_) => {
                    // Sync ConnectionState
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
