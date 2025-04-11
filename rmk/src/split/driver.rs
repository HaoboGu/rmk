//! The abstracted driver layer of the split keyboard.
//!
use core::sync::atomic::Ordering;

use embassy_futures::select::select;
use embassy_time::{Instant, Timer};

use super::SplitMessage;
use crate::channel::{EVENT_CHANNEL, KEY_EVENT_CHANNEL};
use crate::event::{Event, KeyEvent};
use crate::input_device::InputDevice;
use crate::CONNECTION_STATE;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum SplitDriverError {
    SerialError,
    EmptyMessage,
    DeserializeError,
    SerializeError,
    BleError(u8),
    Disconnected,
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
    pub(crate) async fn run(mut self) {
        CONNECTION_STATE.store(true, Ordering::Release);
        let mut conn_state = CONNECTION_STATE.load(Ordering::Acquire);
        // Send connection state once on start
        if let Err(e) = self.receiver.write(&SplitMessage::ConnectionState(conn_state)).await {
            match e {
                SplitDriverError::Disconnected => return,
                _ => error!("SplitDriver write error: {:?}", e),
            }
        }

        let mut last_sync_time = Instant::now();

        loop {
            // Calculate the time until the next 1000ms sync
            let elapsed = last_sync_time.elapsed().as_millis() as u64;
            let wait_time = if elapsed >= 1000 { 1 } else { 1000 - elapsed };

            // Read the message from peripheral, or sync the connection state every 1000ms.
            match select(self.read_event(), Timer::after_millis(wait_time)).await {
                // Use built-in channels for split peripherals
                embassy_futures::select::Either::First(event) => match event {
                    Event::Key(key_event) => KEY_EVENT_CHANNEL.send(key_event).await,
                    _ => {
                        if EVENT_CHANNEL.is_full() {
                            let _ = EVENT_CHANNEL.receive().await;
                        }
                        EVENT_CHANNEL.send(event).await;
                    }
                },
                embassy_futures::select::Either::Second(_) => {
                    // Timer elapsed, sync the connection state
                    CONNECTION_STATE.store(true, Ordering::Release);
                    conn_state = CONNECTION_STATE.load(Ordering::Acquire);
                    if let Err(e) = self.receiver.write(&SplitMessage::ConnectionState(conn_state)).await {
                        match e {
                            SplitDriverError::Disconnected => return,
                            _ => error!("SplitDriver write error: {:?}", e),
                        }
                    }
                    last_sync_time = Instant::now();
                }
            }
        }
    }
}

impl<
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        R: SplitReader + SplitWriter,
    > InputDevice for PeripheralManager<ROW, COL, ROW_OFFSET, COL_OFFSET, R>
{
    async fn read_event(&mut self) -> Event {
        loop {
            match self.receiver.read().await {
                Ok(SplitMessage::Key(e)) => {
                    // Verify the row/col
                    if e.row as usize > ROW || e.col as usize > COL {
                        error!("Invalid peripheral row/col: {} {}", e.row, e.col);
                        continue;
                    }

                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        // Only when the connection is established, send the key event.
                        let adjusted_key_event = KeyEvent {
                            row: e.row + ROW_OFFSET as u8,
                            col: e.col + COL_OFFSET as u8,
                            pressed: e.pressed,
                        };
                        return Event::Key(adjusted_key_event);
                    } else {
                        warn!("Key event from peripheral is ignored because the connection is not established.");
                    }
                }
                Ok(SplitMessage::Event(event)) => {
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        return event;
                    } else {
                        warn!("Event from peripheral is ignored because the connection is not established.");
                    }
                }
                Ok(_) => {
                    // Ignore other types of messages
                    debug!("Ignored non-event split message");
                }
                Err(e) => {
                    error!("Peripheral message read error: {:?}", e);
                }
            }
        }
    }
}
