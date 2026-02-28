//! The abstracted driver layer of the split keyboard.
//!
use core::sync::atomic::Ordering;

use embassy_futures::select::{Either3, Either4, select3, select4};
use embassy_time::Instant;
#[cfg(all(feature = "storage", feature = "_ble"))]
use {crate::channel::FLASH_CHANNEL, crate::split::ble::PeerAddress, crate::storage::FlashOperationMessage};

use super::SplitMessage;
use crate::CONNECTION_STATE;
#[cfg(feature = "_ble")]
use crate::event::PeripheralBatteryEvent;
use crate::event::{KeyboardEvent, KeyboardEventPos, SubscribableEvent, publish_event, publish_event_async};

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
    /// The manager receives from the peripheral and publishes input events.
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
        let mut keyboard_indicator_sub = crate::event::LedIndicatorEvent::subscriber();
        let mut layer_sub = crate::event::LayerChangeEvent::subscriber();
        #[cfg(feature = "_ble")]
        let mut clear_peer_sub = crate::event::ClearPeerEvent::subscriber();
        #[cfg(feature = "split")]
        let mut forward_sub = crate::split::forward::SPLIT_FORWARD_CHANNEL.subscriber().unwrap();

        loop {
            // Calculate the time until the next 3000ms sync
            use embassy_time::Timer;
            let elapsed = last_sync_time.elapsed().as_millis();
            let wait_time = if elapsed >= 3000 { 1 } else { 3000 - elapsed };
            #[cfg(feature = "split")]
            let forward_event = forward_sub.next_message_pure();
            #[cfg(not(feature = "split"))]
            let forward_event = core::future::pending::<crate::split::SplitUserPacket>();
            match select3(
                self.transceiver.read(),
                select4(
                    keyboard_indicator_sub.next_event(),
                    layer_sub.next_event(),
                    forward_event,
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
                    }
                    Err(e) => {
                        error!("Peripheral message read error: {:?}", e);
                    }
                },
                Either3::Second(e) => {
                    let message_to_peri = match e {
                        Either4::First(indicator_event) => {
                            SplitMessage::KeyboardIndicator(indicator_event.indicator.into_bits())
                        }
                        Either4::Second(layer_event) => SplitMessage::Layer(layer_event.layer),
                        Either4::Third(user_packet) => SplitMessage::User(user_packet),
                        #[cfg(feature = "_ble")]
                        Either4::Fourth(_clear_peer) => {
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
        trace!("Got message from peripheral: {:?}", split_message);
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
                        publish_event_async(adjusted_key_event).await;
                    } else {
                        warn!("Key event from peripheral is ignored because the connection is not established.");
                    }
                }
                _ => {
                    // For rotary encoder
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        // Only when the connection is established, send the key event.
                        publish_event_async(e).await;
                    }
                }
            },
            // Process other split messages which requires connection to host
            _ if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) => match split_message {
                // Non-key events are drop-on-full to keep the split read loop responsive.
                SplitMessage::Pointing(e) => publish_event(e),
                #[cfg(feature = "_ble")]
                SplitMessage::BatteryState(state) => {
                    // Publish as PeripheralBatteryEvent with the full state
                    publish_event(PeripheralBatteryEvent { id: self.id, state })
                }
                #[cfg(feature = "split")]
                SplitMessage::User(packet) => {
                    // Wrap with peripheral id and dispatch to local subscribers
                    crate::split::forward::SPLIT_DISPATCH_CHANNEL
                        .immediate_publisher()
                        .publish_immediate(crate::split::DispatchedSplitPacket {
                            peripheral_id: self.id as u8,
                            packet,
                        });
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
