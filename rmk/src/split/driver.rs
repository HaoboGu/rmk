//! The abstracted driver layer of the split keyboard.
//!
use core::sync::atomic::Ordering;

use embassy_futures::select::{select3, Either3};
use embassy_time::{Instant, Timer};
#[cfg(all(feature = "storage", feature = "_ble"))]
use {crate::channel::FLASH_CHANNEL, crate::split::ble::PeerAddress, crate::storage::FlashOperationMessage};

use super::SplitMessage;
use crate::channel::{EVENT_CHANNEL, KEY_EVENT_CHANNEL, SPLIT_MESSAGE_PUBLISHER};
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
    T: SplitReader + SplitWriter,
> {
    /// Receiver
    transceiver: T,
    /// Peripheral id
    id: usize,
}

impl<
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        T: SplitReader + SplitWriter,
    > PeripheralManager<ROW, COL, ROW_OFFSET, COL_OFFSET, T>
{
    pub(crate) fn new(transceiver: T, id: usize) -> Self {
        Self { transceiver, id }
    }

    /// Run the manager.
    ///
    /// The manager receives from the peripheral and forward the message to `KEY_EVENT_CHANNEL`.
    /// It also sync the `ConnectionState` to the peripheral periodically.
    pub(crate) async fn run(mut self) {
        let mut conn_state = CONNECTION_STATE.load(Ordering::Acquire);
        // Send connection state once on start
        if let Err(e) = self.transceiver.write(&SplitMessage::ConnectionState(conn_state)).await {
            match e {
                SplitDriverError::Disconnected => return,
                _ => error!("SplitDriver write error: {:?}", e),
            }
        }

        let mut last_sync_time = Instant::now();
        let mut subscriber = SPLIT_MESSAGE_PUBLISHER
            .subscriber()
            .expect("Failed to create split message subscriber: MaximumSubscribersReached");

        loop {
            // Calculate the time until the next 3000ms sync
            let elapsed = last_sync_time.elapsed().as_millis() as u64;
            let wait_time = if elapsed >= 3000 { 1 } else { 3000 - elapsed };

            // Read the message from peripheral, or sync the connection state every 1000ms.
            match select3(
                self.read_event(),
                subscriber.next_message_pure(),
                Timer::after_millis(wait_time),
            )
            .await
            {
                Either3::First(event) => match event {
                    Event::Key(key_event) => KEY_EVENT_CHANNEL.send(key_event).await,
                    _ => {
                        if EVENT_CHANNEL.is_full() {
                            let _ = EVENT_CHANNEL.receive().await;
                        }
                        EVENT_CHANNEL.send(event).await;
                    }
                },
                Either3::Second(split_message) => {
                    #[cfg(all(feature = "storage", feature = "_ble"))]
                    match split_message {
                        SplitMessage::ClearPeer => {
                            // Clear the peer address
                            FLASH_CHANNEL
                                .send(FlashOperationMessage::PeerAddress(PeerAddress::new(
                                    self.id as u8,
                                    false,
                                    [0; 6],
                                )))
                                .await;
                        }
                        _ => (),
                    }
                    debug!("Publishing split message {:?} to peripherals", split_message);
                    if let Err(e) = self.transceiver.write(&split_message).await {
                        match e {
                            SplitDriverError::Disconnected => return,
                            _ => error!("SplitDriver write error: {:?}", e),
                        }
                    }
                }
                Either3::Third(_) => {
                    // Timer elapsed, sync the connection state
                    conn_state = CONNECTION_STATE.load(Ordering::Acquire);
                    trace!("Syncing connection state to peripheral: {}", conn_state);
                    if let Err(e) = self.transceiver.write(&SplitMessage::ConnectionState(conn_state)).await {
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
            match self.transceiver.read().await {
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
