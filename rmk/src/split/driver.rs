//! The abstracted driver layer of the split keyboard.
//!
use core::sync::atomic::Ordering;

use embassy_futures::select::{Either3, select3};
use embassy_time::Instant;
#[cfg(all(feature = "storage", feature = "_ble"))]
use {crate::channel::FLASH_CHANNEL, crate::split::ble::PeerAddress, crate::storage::FlashOperationMessage};

use super::SplitMessage;
use crate::CONNECTION_STATE;
use crate::event::{ControllerEvent, KeyboardEvent, KeyboardEventPos, publish_input_event, publish_input_event_async};
#[cfg(feature = "_ble")]
use crate::event::{PeripheralBatteryEvent, publish_controller_event};

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

impl<const ROW: usize, const COL: usize, const ROW_OFFSET: usize, const COL_OFFSET: usize, T: SplitReader + SplitWriter>
    PeripheralManager<ROW, COL, ROW_OFFSET, COL_OFFSET, T>
{
    pub(crate) fn new(transceiver: T, id: usize) -> Self {
        Self { transceiver, id }
    }

    /// Run the manager.
    ///
    /// The manager receives from the peripheral and publishes input/controller events.
    /// It also sync the `ConnectionState` to the peripheral periodically.
    pub(crate) async fn run(mut self) {
        use crate::event::EventSubscriber;

        let mut conn_state = CONNECTION_STATE.load(Ordering::Acquire);
        // Send connection state once on start
        if let Err(e) = self.transceiver.write(&SplitMessage::ConnectionState(conn_state)).await {
            match e {
                SplitDriverError::Disconnected => return,
                _ => error!("SplitDriver write error: {:?}", e),
            }
        }

        let mut last_sync_time = Instant::now();
        let mut keyboard_indicator_sub = crate::event::LedIndicatorEvent::controller_subscriber();
        let mut layer_sub = crate::event::LayerChangeEvent::controller_subscriber();
        #[cfg(feature = "_ble")]
        let mut clear_peer_sub = crate::event::ClearPeerEvent::controller_subscriber();

        loop {
            // Calculate the time until the next 3000ms sync
            use embassy_time::Timer;
            let elapsed = last_sync_time.elapsed().as_millis();
            let wait_time = if elapsed >= 3000 { 1 } else { 3000 - elapsed };
            match select3(
                self.transceiver.read(),
                select3(
                    keyboard_indicator_sub.next_event(),
                    layer_sub.next_event(),
                    #[cfg(feature = "_ble")]
                    clear_peer_sub.next_event(),
                    #[cfg(not(feature = "_ble"))]
                    core::future::pending::<()>(),
                ),
                Timer::after_millis(wait_time),
            )
            .await
            {
                Either3::First(read_result) => match read_result {
                    Ok(split_message) => {
                        self.process_peripheral_message(split_message).await;

                        if let Some(indicator_event) = keyboard_indicator_sub.try_next_message_pure() {
                            let message_to_peri =
                                SplitMessage::KeyboardIndicator(indicator_event.indicator.into_bits());
                            debug!("Sending message to peripheral {}: {:?}", self.id, message_to_peri);
                            if let Err(e) = self.transceiver.write(&message_to_peri).await {
                                match e {
                                    SplitDriverError::Disconnected => return,
                                    _ => error!("SplitDriver write error: {:?}", e),
                                }
                            }
                        } else if let Some(layer_event) = layer_sub.try_next_message_pure() {
                            let message_to_peri = SplitMessage::Layer(layer_event.layer);
                            debug!("Sending message to peripheral {}: {:?}", self.id, message_to_peri);
                            if let Err(e) = self.transceiver.write(&message_to_peri).await {
                                match e {
                                    SplitDriverError::Disconnected => return,
                                    _ => error!("SplitDriver write error: {:?}", e),
                                }
                            }
                        }

                        #[cfg(feature = "_ble")]
                        if clear_peer_sub.try_next_message_pure().is_some() {
                            #[cfg(feature = "storage")]
                            // Clear the peer address in storage
                            FLASH_CHANNEL
                                .send(FlashOperationMessage::PeerAddress(PeerAddress::new(
                                    self.id as u8,
                                    false,
                                    [0; 6],
                                )))
                                .await;

                            let message_to_peri = SplitMessage::ClearPeer;
                            debug!("Sending message to peripheral {}: {:?}", self.id, message_to_peri);
                            if let Err(e) = self.transceiver.write(&message_to_peri).await {
                                match e {
                                    SplitDriverError::Disconnected => return,
                                    _ => error!("SplitDriver write error: {:?}", e),
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Peripheral message read error: {:?}", e);
                    }
                },
                Either3::Second(e) => {
                    let message_to_peri = match e {
                        Either3::First(indicator_event) => {
                            SplitMessage::KeyboardIndicator(indicator_event.indicator.into_bits())
                        }
                        Either3::Second(layer_event) => SplitMessage::Layer(layer_event.layer),
                        #[cfg(feature = "_ble")]
                        Either3::Third(_clear_peer) => {
                            #[cfg(feature = "storage")]
                            // Clear the peer address in storage
                            FLASH_CHANNEL
                                .send(FlashOperationMessage::PeerAddress(PeerAddress::new(
                                    self.id as u8,
                                    false,
                                    [0; 6],
                                )))
                                .await;

                            SplitMessage::ClearPeer
                        }
                        #[cfg(not(feature = "_ble"))]
                        _ => continue,
                    };
                    // Send message to peripheral
                    debug!("Sending message to peripheral {}: {:?}", self.id, message_to_peri);
                    if let Err(e) = self.transceiver.write(&message_to_peri).await {
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

    /// Process a single message from the peripheral.
    async fn process_peripheral_message(&self, split_message: SplitMessage) {
        match split_message {
            SplitMessage::Key(e) => match e.pos {
                KeyboardEventPos::Key(key_pos) => {
                    // Verify the row/col
                    if key_pos.row as usize > ROW || key_pos.col as usize > COL {
                        error!("Invalid peripheral row/col: {} {}", key_pos.row, key_pos.col);
                        return;
                    }

                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        // Only when the connection is established, send the key event.
                        let adjusted_key_event = KeyboardEvent::key(
                            key_pos.row + ROW_OFFSET as u8,
                            key_pos.col + COL_OFFSET as u8,
                            e.pressed,
                        );
                        publish_input_event_async(adjusted_key_event).await;
                    } else {
                        warn!("Key event from peripheral is ignored because the connection is not established.");
                    }
                }
                _ => {
                    // For rotary encoder
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        // Only when the connection is established, send the key event.
                        publish_input_event_async(e).await;
                    }
                }
            },
            // Process other split messages which requires connection to host
            _ if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) => match split_message {
                // Non-key events are drop-on-full to keep the split read loop responsive.
                SplitMessage::Touchpad(e) => publish_input_event(e),
                SplitMessage::Pointing(e) => publish_input_event(e),
                #[cfg(feature = "_ble")]
                SplitMessage::BatteryState(state) => {
                    // Publish as PeripheralBatteryEvent with the full state
                    publish_controller_event(PeripheralBatteryEvent { id: self.id, state })
                }
                _ => warn!("{:?} should not come from peripheral", split_message),
            },
            _ => warn!(
                "{:?} from peripheral is ignored because the connection is not established.",
                split_message
            ),
        }
    }
}
