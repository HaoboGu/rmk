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
    /// Running CRC of the current DFU passthrough transfer.
    /// Updated with each forwarded chunk; compared against the peripheral's
    /// DFU-partition CRC when the download completes.
    #[cfg(feature = "dfu_split")]
    passthrough_crc: crate::crc32::Crc32,
}

impl<const ROW: usize, const COL: usize, const ROW_OFFSET: usize, const COL_OFFSET: usize, T: SplitReader + SplitWriter>
    PeripheralManager<ROW, COL, ROW_OFFSET, COL_OFFSET, T>
{
    pub(crate) fn new(transceiver: T, id: usize) -> Self {
        Self {
            transceiver,
            id,
            #[cfg(feature = "dfu_split")]
            passthrough_crc: crate::crc32::Crc32::new(),
        }
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
            // Passthrough DFU commands take priority over normal
            // peripheral events so that USB control transfers are
            // not starved while forwarding chunks.  While we are
            // inside `handle_passthrough` the host polls GETSTATUS
            // and gets `dfuDNBUSY` — flow control is built in.
            #[cfg(feature = "dfu_split")]
            if crate::dfu::passthrough_pending(self.id) {
                self.handle_passthrough().await;
                continue;
            }

            // Race the next event-to-send against a short timer so the
            // loop periodically returns control — needed so the
            // passthrough check above can see newly queued chunks.
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

            // Race events against a short timer so we periodically wake up
            // and can check for DFU passthrough commands.
            let event_or_timer = select(
                next_event_to_peri,
                embassy_time::Timer::after_millis(5),
            );

            match select(self.transceiver.read(), event_or_timer).await {
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
                Either::Second(Either::First(msg)) => {
                    if self.send(&msg).await.is_err() {
                        return;
                    }
                }
                Either::Second(Either::Second(())) => {
                    // Timer expired — check passthrough at top of next iteration
                }
            }
        }
    }

    /// Handle a proactive `FirmwareHashResponse` received in the main event
    /// loop (i.e. *after* the initial `check_firmware_update` already timed
    /// out because the peripheral was not yet booted).
    #[cfg(feature = "dfu_split")]
    async fn handle_proactive_hash(&mut self, hash: u32) {
        let (firmware, expected_hash) = match crate::dfu::get_firmware_update_data(self.id) {
            Some(d) => d,
            None => {
                info!("dfu_split: no firmware data set for peripheral {}, skipping proactive hash check", self.id);
                return;
            }
        };

        info!("dfu_split: proactive hash from peripheral ({:#x}), checking...", hash);

        // Only `check_firmware_update` (initial connect) can be force-bypassed;
        // the proactive handler always checks the hash to avoid boot loops.
        if hash == expected_hash {
            info!("dfu_split: firmware hash matches (peripheral={:#x}, expected={:#x}), no update needed", hash, expected_hash);
            return;
        }

        let len = firmware.len();
        info!(
            "dfu_split: firmware hash mismatch (peripheral={:#x}, expected={:#x}), starting update ({} bytes)",
            hash, expected_hash, len,
        );
        self.send_firmware_update(firmware, expected_hash).await;
    }

    /// Process all pending DFU passthrough commands.
    ///
    /// Called from the main event loop whenever [`crate::dfu::passthrough_pending`]
    /// returns true.  Drains the entire queue before returning:
    ///
    /// * **Chunk** — sends it to the peripheral via `FirmwareChunk`, waits
    ///   for the `FirmwareChunkAck`, and accumulates a running CRC‑32.
    /// * **Finish** — all chunks sent; asks the peripheral to compute the
    ///   DFU partition CRC, compares it against the central's accumulated
    ///   CRC, and either confirms (`FirmwareCrcOk`) or aborts (`FirmwareCrcFail`).
    ///
    /// The running CRC lives in [`PeripheralManager::passthrough_crc`] so
    /// that it survives across invocations (the queue is drained incrementally).
    ///
    /// # Flow control
    ///
    /// While this method is executing the host polls GETSTATUS.
    /// [`RmkDfuInterface::control_in`] sees that [`PASSTHROUGH_TARGET`]
    /// is still busy and returns `dfuDNBUSY`, so the host waits 50 ms and
    /// polls again.  Once the last chunk is acked and
    /// [`passthrough_done_if_empty`] clears the target, the real DFU state
    /// is reported and the host sends the next DNLOAD block immediately.
    ///
    /// This prevents the host from overrunning the split-link throughput
    /// while keeping the USB ISR completely non-blocking.
    #[cfg(feature = "dfu_split")]
    async fn handle_passthrough(&mut self) {
        use embassy_time::{Duration, Timer};

        while let Some(cmd) = crate::dfu::passthrough_take_command() {
            match cmd {
            crate::dfu::PassthroughCommand::Chunk(chunk) => {
                debug!("dfu_split/passthrough: sending chunk @ offset {} ({} bytes)", chunk.offset, chunk.len);
                self.passthrough_crc.update(&chunk.data[..chunk.len as usize]);
                let msg = SplitMessage::FirmwareChunk {
                    offset: chunk.offset,
                    len: chunk.len,
                    data: super::FirmwareChunkData(chunk.data),
                };
                if self.send(&msg).await.is_err() {
                    error!("dfu_split/passthrough: disconnected during chunk send");
                    crate::dfu::passthrough_done_if_empty();
                    return;
                }

                // Wait for the peripheral to acknowledge this chunk
                loop {
                    match select(self.transceiver.read(), Timer::after(Duration::from_secs(2))).await {
                        Either::First(Ok(SplitMessage::FirmwareChunkAck { offset, .. })) if offset == chunk.offset => {
                            break;
                        }
                        Either::First(Ok(_)) => {}
                        Either::First(Err(e)) => {
                            error!("dfu_split/passthrough: read error: {:?}", e);
                            break;
                        }
                        Either::Second(_) => {
                            error!("dfu_split/passthrough: timeout waiting for chunk ack");
                            break;
                        }
                    }
                }

                crate::dfu::passthrough_done_if_empty();
            }
            crate::dfu::PassthroughCommand::Finish => {
                info!("dfu_split/passthrough: DFU download complete, starting end-to-end verification");

                // Tell the peripheral to finalize and compute its CRC
                if self.send(&SplitMessage::FirmwareUpdateComplete).await.is_err() {
                    error!("dfu_split/passthrough: disconnected during finish");
                    crate::dfu::passthrough_done_if_empty();
                    return;
                }

                // Wait for CRC report from peripheral
                let crc = loop {
                    match select(self.transceiver.read(), Timer::after(Duration::from_secs(5))).await {
                        Either::First(Ok(SplitMessage::FirmwareCrcReport(crc))) => break Some(crc),
                        Either::First(Ok(_)) => {}
                        Either::First(Err(e)) => {
                            error!("dfu_split/passthrough: read error during CRC: {:?}", e);
                            break None;
                        }
                        Either::Second(_) => {
                            error!("dfu_split/passthrough: timeout waiting for CRC");
                            break None;
                        }
                    }
                };

                let Some(peripheral_crc) = crc else {
                    error!("dfu_split/passthrough: CRC verification failed — no response from peripheral");
                    self.send(&SplitMessage::FirmwareCrcFail).await.ok();
                    crate::dfu::passthrough_done_if_empty();
                    return;
                };

                let central_crc = self.passthrough_crc.finalize();
                self.passthrough_crc = crate::crc32::Crc32::new(); // reset for next transfer

                if central_crc != peripheral_crc {
                    error!(
                        "dfu_split/passthrough: CRC mismatch (central={:#010x}, peripheral={:#010x}) — firmware corrupt",
                        central_crc, peripheral_crc
                    );
                    self.send(&SplitMessage::FirmwareCrcFail).await.ok();
                    crate::dfu::passthrough_done_if_empty();
                    return;
                }

                info!("dfu_split/passthrough: CRC OK (central={:#010x}), confirming update", central_crc);
                if self.send(&SplitMessage::FirmwareCrcOk).await.is_err() {
                    error!("dfu_split/passthrough: disconnected during CRC OK");
                    crate::dfu::passthrough_done_if_empty();
                    return;
                }

                // Wait for peripheral to mark updated and reset
                loop {
                    match select(self.transceiver.read(), Timer::after(Duration::from_secs(2))).await {
                        Either::First(Ok(SplitMessage::FirmwareUpdateConfirm)) => {
                            info!("dfu_split/passthrough: peripheral confirmed, update complete");
                            break;
                        }
                        Either::First(Ok(_)) => {}
                        Either::First(Err(e)) => {
                            error!("dfu_split/passthrough: read error during confirm: {:?}", e);
                            break;
                        }
                        Either::Second(_) => {
                            info!("dfu_split/passthrough: timeout on confirm (peripheral may have reset)");
                            break;
                        }
                    }
                }

                crate::dfu::passthrough_done_if_empty();
            }
        }
        }
    }

    /// Check and optionally update the peripheral's firmware.
    #[cfg(feature = "dfu_split")]
    async fn check_firmware_update(&mut self) {
        use embassy_time::{Duration, Timer};

        let (firmware, expected_hash) = match crate::dfu::get_firmware_update_data(self.id) {
            Some(d) => d,
            None => {
                info!("dfu_split: no firmware data set for peripheral {}, skipping update check", self.id);
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
            info!("dfu_split: firmware hash matches (peripheral={:#x}, expected={:#x}), no update needed", peripheral_hash, expected_hash);
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
    ///
    /// 1. Per-chunk CRC-32 verification: each Ack carries `CRC32(chunk_data)`;
    ///    mismatch → retry that chunk (up to 3 tries).
    /// 2. End-to-end verification: peripheral reads back its DFU partition,
    ///    sends the CRC-32, central compares against `expected_hash`.
    ///    Mismatch → retry the entire transfer (up to 3 attempts).
    #[cfg(feature = "dfu_split")]
    async fn send_firmware_update(&mut self, firmware: &[u8], expected_hash: u32) {
        use crate::crc32::Crc32;
        use crate::dfu::with_led;
        use embassy_time::{Duration, Timer};
        const MAX_RETRIES: u32 = 3;
        const MAX_ATTEMPTS: u32 = 3;

        with_led(|led| led.set_high());

        for attempt in 1..=MAX_ATTEMPTS {
            info!("dfu_split: update attempt {}/{}", attempt, MAX_ATTEMPTS);

            let mut central_crc = Crc32::new();
            let mut all_acked = true;

            // --- send all chunks, verifying per-chunk CRC on each Ack ---
            for (offset, chunk) in firmware.chunks(256).enumerate() {
                let offset_bytes = (offset * 256) as u32;
                let mut data = [0u8; 256];
                data[..chunk.len()].copy_from_slice(chunk);

                let chunk_crc = crate::crc32::crc32(&data[..chunk.len()]);
                central_crc.update(&data[..chunk.len()]);

                let mut retries = 0;
                let mut acked = false;

                while !acked && retries < MAX_RETRIES {
                    if retries > 0 {
                        info!("dfu_split: retry {}/{} for chunk at offset {}", retries + 1, MAX_RETRIES, offset_bytes);
                    }

                    debug!("dfu_split: sending chunk at offset {} ({} bytes)", offset_bytes, chunk.len());
                    if self.send(&SplitMessage::FirmwareChunk { offset: offset_bytes, len: chunk.len() as u16, data: super::FirmwareChunkData(data) }).await.is_err() {
                        error!("dfu_split: disconnected during chunk send");
                        with_led(|led| led.set_low());
                        return;
                    }

                    let got = loop {
                        match select(self.transceiver.read(), Timer::after(Duration::from_secs(2))).await {
                            Either::First(Ok(SplitMessage::FirmwareChunkAck { offset: ack_offset, crc: ack_crc })) => {
                                if ack_offset == offset_bytes {
                                    if ack_crc == chunk_crc {
                                        break true;
                                    }
                                    warn!("dfu_split: per-chunk CRC mismatch at offset {} (peripheral={:#010x}, central={:#010x})",
                                        offset_bytes, ack_crc, chunk_crc);
                                    break false;
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
                    error!("dfu_split: chunk at offset {} failed after {} retries", offset_bytes, MAX_RETRIES);
                    all_acked = false;
                    break;
                }
            }

            if !all_acked {
                continue; // outer retry loop
            }

            // --- local sanity check: central_crc should match expected_hash ---
            let local_crc = central_crc.finalize();
            if local_crc != expected_hash {
                error!("dfu_split: central CRC mismatch (computed={:#010x}, expected={:#010x}) — aborting",
                    local_crc, expected_hash);
                with_led(|led| led.set_low());
                return; // not a transmission error, something is wrong with the binary
            }

            // --- end-to-end: ask peripheral to verify DFU partition CRC ---
            info!("dfu_split: all chunks sent, requesting DFU CRC verification");
            if self.send(&SplitMessage::FirmwareUpdateComplete).await.is_err() {
                error!("dfu_split: disconnected during update complete signal");
                with_led(|led| led.set_low());
                return;
            }

            let peripheral_crc = loop {
                match select(self.transceiver.read(), Timer::after(Duration::from_secs(5))).await {
                    Either::First(Ok(SplitMessage::FirmwareCrcReport(crc))) => {
                        break Some(crc);
                    }
                    Either::First(Ok(other)) => {
                        info!("dfu_split: waiting for DFU CRC report, got {:?}", other);
                    }
                    Either::First(Err(e)) => {
                        error!("dfu_split: read error during CRC report: {:?}", e);
                        break None;
                    }
                    Either::Second(_) => {
                        error!("dfu_split: timeout waiting for DFU CRC report");
                        break None;
                    }
                }
            };

            let Some(dfu_crc) = peripheral_crc else {
                continue; // retry
            };

            if dfu_crc == expected_hash {
                info!("dfu_split: end-to-end CRC matches (peripheral={:#010x}, central={:#010x}), confirming update", dfu_crc, expected_hash);
                if self.send(&SplitMessage::FirmwareCrcOk).await.is_err() {
                    error!("dfu_split: disconnected during CRC OK send");
                    with_led(|led| led.set_low());
                    return;
                }

                // Wait for peripheral confirm
                loop {
                    match select(self.transceiver.read(), Timer::after(Duration::from_secs(2))).await {
                        Either::First(Ok(SplitMessage::FirmwareUpdateConfirm)) => {
                            info!("dfu_split: peripheral confirmed, update complete");
                            with_led(|led| led.set_low());
                            return;
                        }
                        Either::First(Ok(other)) => {
                            info!("dfu_split: waiting for confirm, got {:?}", other);
                        }
                        Either::First(Err(e)) => {
                            error!("dfu_split: read error during confirm: {:?}", e);
                            with_led(|led| led.set_low());
                            return;
                        }
                        Either::Second(_) => {
                            error!("dfu_split: timeout waiting for firmware update confirm");
                            // Peripheral may have already reset
                            with_led(|led| led.set_low());
                            return;
                        }
                    }
                }
            } else {
                warn!("dfu_split: end-to-end CRC mismatch (peripheral={:#010x}, expected={:#010x}), retrying",
                    dfu_crc, expected_hash);
                if self.send(&SplitMessage::FirmwareCrcFail).await.is_err() {
                    error!("dfu_split: disconnected during CRC fail send");
                    with_led(|led| led.set_low());
                    return;
                }
                Timer::after(Duration::from_millis(100)).await;
            }
        }

        error!("dfu_split: all {} update attempts failed, giving up", MAX_ATTEMPTS);
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
            SplitMessage::FirmwareChunkAck { offset, crc: _ } => {
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
