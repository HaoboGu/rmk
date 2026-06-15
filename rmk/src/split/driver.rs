//! The abstracted driver layer of the split keyboard.
//!
use embassy_futures::select::{Either, select};
use futures::FutureExt;

use super::SplitMessage;
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
    /// It also syncs the central's `ConnectionStatus` to the peripheral on every
    /// change as an informational signal
    pub(crate) async fn run(mut self) {
        use crate::event::EventSubscriber;

        let mut indicator_sub = crate::event::LedIndicatorEvent::subscriber();
        let mut layer_sub = crate::event::LayerChangeEvent::subscriber();
        // Subscribe before the initial send so any change racing past the
        // snapshot is still delivered to us.
        let mut connection_sub = crate::event::ConnectionStatusChangeEvent::subscriber();
        #[cfg(feature = "_ble")]
        let mut clear_peer_sub = crate::event::ClearPeerEvent::subscriber();

        #[cfg(feature = "display")]
        let mut wpm_sub = crate::event::WpmUpdateEvent::subscriber();
        #[cfg(feature = "display")]
        let mut modifier_sub = crate::event::ModifierEvent::subscriber();
        #[cfg(feature = "display")]
        let mut sleep_sub = crate::event::SleepStateEvent::subscriber();

        // Send the current state once on startup so the peripheral matches us
        // even when no transition has happened since the central booted.
        if self
            .send(&SplitMessage::ConnectionStatus(
                crate::state::current_connection_status(),
            ))
            .await
            .is_err()
        {
            return;
        }

        // Check for peripheral firmware update
        #[cfg(feature = "dfu_split")]
        self.check_firmware_update().await;

        loop {
            // Use select_biased_with_feature to handle feature-gated subscriber arms
            let next_event_to_peri = async {
                crate::select_biased_with_feature! {
                    e = indicator_sub.next_event().fuse() => SplitMessage::KeyboardIndicator(e.0.into_bits()),
                    e = layer_sub.next_event().fuse() => SplitMessage::Layer(e.0),
                    e = connection_sub.next_event().fuse() => SplitMessage::ConnectionStatus(e.0),
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

            match select(self.transceiver.read(), next_event_to_peri).await {
                Either::First(read_result) => match read_result {
                    #[cfg(feature = "dfu_split")]
                    Ok(SplitMessage::FirmwareHashResponse(hash)) => {
                        self.handle_proactive_hash(hash).await;
                    }
                    Ok(split_message) => {
                        self.process_peripheral_message(split_message).await;
                    }
                    Err(e) => {
                        error!("Peripheral message read error: {:?}", e);
                    }
                },
                Either::Second(msg) => {
                    if self.send(&msg).await.is_err() {
                        return;
                    }
                }
            }
        }
    }

    /// Handle a proactive `FirmwareHashResponse` received in the main event
    /// loop (i.e. *after* the initial `check_firmware_update` already timed
    /// out because the peripheral was not yet booted).
    #[cfg(feature = "dfu_split")]
    async fn handle_proactive_hash(&mut self, hash: u32) {
        let (firmware, expected_hash) = match crate::dfu::get_firmware_update_data() {
            Some(d) => d,
            None => {
                info!("dfu_split: no firmware data set, skipping proactive hash check");
                return;
            }
        };

        info!("dfu_split: proactive hash from peripheral ({:#x}), checking...", hash);

        // Only `check_firmware_update` (initial connect) can be force-bypassed;
        // the proactive handler always checks the hash to avoid boot loops.
        if hash == expected_hash {
            info!("dfu_split: peripheral firmware hash matches ({:#x}), no update needed", hash);
            return;
        }

        let len = firmware.len();
        info!(
            "dfu_split: firmware hash mismatch (peripheral={:#x}, expected={:#x}), starting update ({} bytes)",
            hash, expected_hash, len,
        );
        self.send_firmware_update(firmware, expected_hash).await;
    }

    /// Check and optionally update the peripheral's firmware.
    #[cfg(feature = "dfu_split")]
    async fn check_firmware_update(&mut self) {
        use embassy_time::{Duration, Timer};

        let (firmware, expected_hash) = match crate::dfu::get_firmware_update_data() {
            Some(d) => d,
            None => {
                info!("dfu_split: no firmware data set, skipping update check");
                return;
            }
        };

        info!("dfu_split: checking peripheral firmware...");

        // Query the peripheral's firmware hash
        if self.send(&SplitMessage::FirmwareHashQuery).await.is_err() {
            error!("dfu_split: disconnected during hash query");
            return;
        }

        let hash_response = loop {
            match select(self.transceiver.read(), Timer::after(Duration::from_secs(2))).await {
                Either::First(Ok(SplitMessage::FirmwareHashResponse(hash))) => break Some(hash),
                Either::First(Ok(other)) => {
                    warn!("dfu_split: unexpected message during hash query: {:?}", other);
                }
                Either::First(Err(e)) => {
                    error!("dfu_split: read error during hash query: {:?}", e);
                    break None;
                }
                Either::Second(_) => break None,
            }
        };

        let peripheral_hash = match hash_response {
            Some(h) => h,
            None => {
                info!("dfu_split: no hash response (peripheral may be on old firmware), starting update");
                self.send_firmware_update(firmware, expected_hash).await;
                return;
            }
        };

        #[cfg(not(feature = "dfu_split_force_update"))]
        if peripheral_hash == expected_hash {
            info!("dfu_split: peripheral firmware hash matches ({:#x}), no update needed", peripheral_hash);
            return;
        }

        #[cfg(feature = "dfu_split_force_update")]
        info!("dfu_split: force update enabled, ignoring hash (peripheral={:#x}, expected={:#x}) match",
            peripheral_hash,
            expected_hash,
        );

        #[cfg(not(feature = "dfu_split_force_update"))]
        info!(
            "dfu_split: firmware hash mismatch (peripheral={:#x}, expected={:#x}), starting update ({} bytes)",
            peripheral_hash,
            expected_hash,
            firmware.len(),
        );
        self.send_firmware_update(firmware, expected_hash).await;
    }

    /// Send the full firmware binary to the peripheral in 256-byte chunks.
    #[cfg(feature = "dfu_split")]
    async fn send_firmware_update(&mut self, firmware: &[u8], expected_hash: u32) {
        use crate::dfu::with_led;
        use embassy_time::{Duration, Timer};
        const MAX_RETRIES: u32 = 3;

        with_led(|led| led.set_high());

        for (offset, chunk) in firmware.chunks(256).enumerate() {
            let offset_bytes = (offset * 256) as u32;
            let mut data = [0u8; 256];
            data[..chunk.len()].copy_from_slice(chunk);

            let mut retries = 0;
            let mut acked = false;

            while !acked && retries < MAX_RETRIES {
                if retries > 0 {
                    info!("dfu_split: retry {}/{} for chunk at offset {}", retries + 1, MAX_RETRIES, offset_bytes);
                }

                info!("dfu_split: sending chunk at offset {} ({} bytes)", offset_bytes, chunk.len());
                if self.send(&SplitMessage::FirmwareChunk { offset: offset_bytes, data: super::FirmwareChunkData(data) }).await.is_err() {
                    error!("dfu_split: disconnected during chunk send");
                    with_led(|led| led.set_low());
                    return;
                }

                let got = loop {
                    match select(self.transceiver.read(), Timer::after(Duration::from_secs(2))).await {
                        Either::First(Ok(SplitMessage::FirmwareChunkAck { offset: ack_offset })) => {
                            if ack_offset == offset_bytes {
                                break true;
                            }
                            info!("dfu_split: got ack for offset {}, waiting for {}", ack_offset, offset_bytes);
                        }
                        Either::First(Ok(other)) => {
                            warn!("dfu_split: unexpected message during chunk transfer: {:?}", other);
                        }
                        Either::First(Err(e)) => {
                            error!("dfu_split: read error during chunk transfer: {:?}", e);
                            break false;
                        }
                        Either::Second(_) => break false,
                    }
                };
                acked = got;
                retries += 1;
            }

            if !acked {
                error!("dfu_split: chunk at offset {} failed after {} retries, aborting", offset_bytes, MAX_RETRIES);
                with_led(|led| led.set_low());
                return;
            }
        }

        info!("dfu_split: all chunks sent, signaling update complete");
        if self.send(&SplitMessage::FirmwareUpdateComplete(expected_hash)).await.is_err() {
            error!("dfu_split: disconnected during update complete signal");
            with_led(|led| led.set_low());
            return;
        }

        // Wait for peripheral to confirm
        loop {
            match select(self.transceiver.read(), Timer::after(Duration::from_secs(2))).await {
                Either::First(Ok(SplitMessage::FirmwareUpdateConfirm)) => {
                    info!("dfu_split: peripheral confirmed, update complete");
                    break;
                }
                Either::First(Ok(other)) => {
                    info!("dfu_split: waiting for confirm, got {:?}", other);
                }
                Either::First(Err(e)) => {
                    error!("dfu_split: read error during confirm: {:?}", e);
                    break;
                }
                Either::Second(_) => {
                    error!("dfu_split: timeout waiting for firmware update confirm");
                    break;
                }
            }
        }

        with_led(|led| led.set_low());
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

                    let adjusted_key_event = KeyboardEvent::key(
                        key_pos.row + ROW_OFFSET as u8,
                        key_pos.col + COL_OFFSET as u8,
                        e.pressed,
                    );
                    publish_event_async(adjusted_key_event).await;
                }
                _ => {
                    // For rotary encoder
                    publish_event_async(e).await;
                }
            },
            // Non-key events are drop-on-full to keep the split read loop responsive.
            SplitMessage::Pointing(e) => publish_event(e),
            #[cfg(feature = "_ble")]
            SplitMessage::BatteryStatus(state) => {
                use crate::event::PeripheralBatteryEvent;
                publish_event(PeripheralBatteryEvent { id: self.id, state })
            }
            #[cfg(feature = "dfu_split")]
            SplitMessage::FirmwareHashResponse(hash) => {
                info!("dfu_split: stale hash response ({:#x}) in event loop, should not happen", hash);
            }
            #[cfg(feature = "dfu_split")]
            SplitMessage::FirmwareChunkAck { offset } => {
                info!("dfu_split: stale chunk ack (offset {}) in event loop, ignoring", offset);
            }
            #[cfg(feature = "dfu_split")]
            SplitMessage::FirmwareUpdateConfirm => {
                info!("dfu_split: stale update confirm in event loop, ignoring");
            }
            _ => warn!("{:?} should not come from peripheral", split_message),
        }
    }
}
