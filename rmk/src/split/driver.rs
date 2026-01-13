//! The abstracted driver layer of the split keyboard.
//!
use core::sync::atomic::Ordering;

use embassy_time::Instant;
#[cfg(all(feature = "storage", feature = "_ble"))]
use {crate::channel::FLASH_CHANNEL, crate::split::ble::PeerAddress, crate::storage::FlashOperationMessage};

use super::SplitMessage;
use crate::CONNECTION_STATE;
use crate::channel::{EVENT_CHANNEL, KEY_EVENT_CHANNEL};
use crate::event::{Event, KeyboardEvent, KeyboardEventPos};
use crate::input_device::InputDevice;

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
    /// The manager receives from the peripheral and forward the message to `KEY_EVENT_CHANNEL`.
    /// It also sync the `ConnectionState` to the peripheral periodically.
    pub(crate) async fn run(mut self) {
        #[cfg(feature = "_ble")]
        use embassy_futures::select::Either4;
        #[cfg(not(feature = "_ble"))]
        use embassy_futures::select::{Either3, select3};
        #[cfg(feature = "_ble")]
        use futures::FutureExt;

        use crate::event::{ControllerEventTrait, EventSubscriber};

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

        loop {
            #[cfg(feature = "_ble")]
            let result = {
                // Calculate the time until the next 3000ms sync
                use embassy_time::Timer;
                let elapsed = last_sync_time.elapsed().as_millis();
                let wait_time = if elapsed >= 3000 { 1 } else { 3000 - elapsed };
                // With BLE, we have 4 sources to select from
                futures::select_biased! {
                    event = self.read_event().fuse() => Either4::First(event),
                    kbd_ind = keyboard_indicator_sub.next_event().fuse() => Either4::Second(kbd_ind),
                    layer = layer_sub.next_event().fuse() => Either4::Third(layer),
                    clear = clear_peer_sub.next_event().fuse() => Either4::Fourth(Some(clear)),
                    _ = Timer::after_millis(wait_time).fuse() => Either4::Fourth(None),
                }
            };

            #[cfg(not(feature = "_ble"))]
            let result = {
                // Without BLE, we only have 3 sources (no clear_peer)
                select3(
                    self.read_event(),
                    keyboard_indicator_sub.next_event(),
                    layer_sub.next_event(),
                )
                .await
            };

            #[cfg(feature = "_ble")]
            match result {
                Either4::First(event) => match event {
                    Event::Key(key_event) => KEY_EVENT_CHANNEL.send(key_event).await,
                    _ => {
                        if EVENT_CHANNEL.is_full() {
                            let _ = EVENT_CHANNEL.receive().await;
                        }
                        EVENT_CHANNEL.send(event).await;
                    }
                },
                Either4::Second(indicator_event) => {
                    // Send KeyboardIndicator state to peripheral
                    debug!(
                        "Sending KeyboardIndicator to peripheral {}: {:?}",
                        self.id, indicator_event.indicator
                    );
                    if let Err(e) = self
                        .transceiver
                        .write(&SplitMessage::KeyboardIndicator(indicator_event.indicator.into_bits()))
                        .await
                    {
                        match e {
                            SplitDriverError::Disconnected => return,
                            _ => error!("SplitDriver write error: {:?}", e),
                        }
                    }
                }
                Either4::Third(layer_event) => {
                    // Send layer number to peripheral
                    debug!("Sending layer number to peripheral {}: {}", self.id, layer_event.layer);
                    if let Err(e) = self.transceiver.write(&SplitMessage::Layer(layer_event.layer)).await {
                        match e {
                            SplitDriverError::Disconnected => return,
                            _ => error!("SplitDriver write error: {:?}", e),
                        }
                    }
                }
                Either4::Fourth(maybe_clear_peer) => {
                    if maybe_clear_peer.is_some() {
                        #[cfg(feature = "storage")]
                        // Clear the peer address in storage
                        FLASH_CHANNEL
                            .send(FlashOperationMessage::PeerAddress(PeerAddress::new(
                                self.id as u8,
                                false,
                                [0; 6],
                            )))
                            .await;

                        // Write `ClearPeer` message to peripheral
                        debug!("Write ClearPeer message to peripheral {}", self.id);
                        if let Err(e) = self.transceiver.write(&SplitMessage::ClearPeer).await {
                            match e {
                                SplitDriverError::Disconnected => return,
                                _ => error!("SplitDriver write error: {:?}", e),
                            }
                        }
                    } else {
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

            #[cfg(not(feature = "_ble"))]
            {
                // Without BLE feature, handle only 3 cases
                match result {
                    Either3::First(event) => match event {
                        Event::Key(key_event) => KEY_EVENT_CHANNEL.send(key_event).await,
                        _ => {
                            if EVENT_CHANNEL.is_full() {
                                let _ = EVENT_CHANNEL.receive().await;
                            }
                            EVENT_CHANNEL.send(event).await;
                        }
                    },
                    Either3::Second(indicator_event) => {
                        // Send KeyboardIndicator state to peripheral
                        debug!(
                            "Sending KeyboardIndicator to peripheral {}: {:?}",
                            self.id, indicator_event.indicator
                        );
                        if let Err(e) = self
                            .transceiver
                            .write(&SplitMessage::KeyboardIndicator(indicator_event.indicator.into_bits()))
                            .await
                        {
                            match e {
                                SplitDriverError::Disconnected => return,
                                _ => error!("SplitDriver write error: {:?}", e),
                            }
                        }
                    }
                    Either3::Third(layer_event) => {
                        // Send layer number to peripheral
                        debug!("Sending layer number to peripheral {}: {}", self.id, layer_event.layer);
                        if let Err(e) = self.transceiver.write(&SplitMessage::Layer(layer_event.layer)).await {
                            match e {
                                SplitDriverError::Disconnected => return,
                                _ => error!("SplitDriver write error: {:?}", e),
                            }
                        }
                    }
                }

                // Check timer separately for non-BLE case
                let elapsed = last_sync_time.elapsed().as_millis();
                if elapsed >= 3000 {
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

impl<const ROW: usize, const COL: usize, const ROW_OFFSET: usize, const COL_OFFSET: usize, R: SplitReader + SplitWriter>
    InputDevice for PeripheralManager<ROW, COL, ROW_OFFSET, COL_OFFSET, R>
{
    async fn read_event(&mut self) -> Event {
        loop {
            match self.transceiver.read().await {
                Ok(SplitMessage::Key(e)) => {
                    match e.pos {
                        KeyboardEventPos::Key(key_pos) => {
                            // Verify the row/col
                            if key_pos.row as usize > ROW || key_pos.col as usize > COL {
                                error!("Invalid peripheral row/col: {} {}", key_pos.row, key_pos.col);
                                continue;
                            }

                            if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                                // Only when the connection is established, send the key event.
                                let adjusted_key_event = KeyboardEvent::key(
                                    key_pos.row + ROW_OFFSET as u8,
                                    key_pos.col + COL_OFFSET as u8,
                                    e.pressed,
                                );
                                return Event::Key(adjusted_key_event);
                            } else {
                                warn!(
                                    "Key event from peripheral is ignored because the connection is not established."
                                );
                            }
                        }
                        _ => {
                            if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                                // Only when the connection is established, send the key event.
                                return Event::Key(e);
                            }
                        }
                    }
                }
                Ok(SplitMessage::Event(event)) => {
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        return event;
                    } else {
                        warn!("Event from peripheral is ignored because the connection is not established.");
                    }
                }
                Ok(SplitMessage::BatteryLevel(level)) => {
                    // Publish peripheral battery level to controller channel when connected
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        crate::event::publish_controller_event(crate::event::PeripheralBatteryEvent {
                            id: self.id,
                            level,
                        });
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
