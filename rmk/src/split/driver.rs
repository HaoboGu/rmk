//! The abstracted driver layer of the split keyboard.
//!
use core::sync::atomic::Ordering;

use embassy_futures::select::{Either3, select3};
use embassy_time::Instant;
use futures::FutureExt;

use super::SplitMessage;
use crate::CONNECTION_STATE;
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

    /// Send a message to the peripheral, returning Err on disconnect.
    async fn send(&mut self, msg: &SplitMessage) -> Result<(), ()> {
        debug!("Sending message to peripheral {}: {:?}", self.id, msg);
        match self.transceiver.write(msg).await {
            Ok(_) => Ok(()),
            Err(SplitDriverError::Disconnected) => Err(()),
            Err(e) => {
                error!("SplitDriver write error: {:?}", e);
                Ok(())
            }
        }
    }

    /// Run the manager.
    ///
    /// The manager receives from the peripheral and publishes input events.
    /// It also syncs the `ConnectionState` to the peripheral periodically.
    pub(crate) async fn run(mut self) {
        use embassy_time::Timer;

        use crate::event::EventSubscriber;

        let mut conn_state = CONNECTION_STATE.load(Ordering::Acquire);
        if self.send(&SplitMessage::ConnectionState(conn_state)).await.is_err() {
            return;
        }

        let mut last_sync_time = Instant::now();
        let mut indicator_sub = crate::event::LedIndicatorEvent::subscriber();
        let mut layer_sub = crate::event::LayerChangeEvent::subscriber();
        #[cfg(feature = "_ble")]
        let mut clear_peer_sub = crate::event::ClearPeerEvent::subscriber();
        #[cfg(feature = "display")]
        let mut wpm_sub = crate::event::WpmUpdateEvent::subscriber();
        #[cfg(feature = "display")]
        let mut modifier_sub = crate::event::ModifierEvent::subscriber();
        #[cfg(feature = "display")]
        let mut sleep_sub = crate::event::SleepStateEvent::subscriber();

        loop {
            let elapsed = last_sync_time.elapsed().as_millis();
            let wait_time = if elapsed >= 3000 { 1 } else { 3000 - elapsed };

            // Use select_biased_with_feature to handle feature-gated subscriber arms
            let next_event_to_peri = async {
                crate::select_biased_with_feature! {
                    e = indicator_sub.next_event().fuse() => SplitMessage::KeyboardIndicator(e.0.into_bits()),
                    e = layer_sub.next_event().fuse() => SplitMessage::Layer(e.0),
                    with_feature("_ble"): _ = clear_peer_sub.next_event().fuse() => {
                        #[cfg(feature = "storage")]
                        {
                            use {crate::channel::FLASH_CHANNEL, crate::split::ble::PeerAddress, crate::storage::FlashOperationMessage};
                            FLASH_CHANNEL
                                .send(FlashOperationMessage::PeerAddress(PeerAddress::new(
                                    self.id as u8,
                                    false,
                                    [0; 6],
                                )))
                                .await;
                        }
                        SplitMessage::ClearPeer
                    },
                    with_feature("display"): e = wpm_sub.next_event().fuse() => SplitMessage::Wpm(e.0),
                    with_feature("display"): e = modifier_sub.next_event().fuse() => SplitMessage::Modifier(e.modifier.into_bits()),
                    with_feature("display"): e = sleep_sub.next_event().fuse() => SplitMessage::SleepState(e.0),
                }
            };

            match select3(
                self.transceiver.read(),
                next_event_to_peri,
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
                Either3::Second(message_to_peri) => {
                    if self.send(&message_to_peri).await.is_err() {
                        return;
                    }
                }
                Either3::Third(_) => {
                    conn_state = CONNECTION_STATE.load(Ordering::Acquire);
                    trace!("Syncing connection state to peripheral: {}", conn_state);
                    if self.send(&SplitMessage::ConnectionState(conn_state)).await.is_err() {
                        return;
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
                    if key_pos.row as usize >= ROW || key_pos.col as usize >= COL {
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
                SplitMessage::BatteryStatus(state) => {
                    // Publish as PeripheralBatteryEvent with the full state
                    use crate::event::PeripheralBatteryEvent;
                    publish_event(PeripheralBatteryEvent { id: self.id, state })
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
