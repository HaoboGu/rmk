//! Rynk host service — RMK-native protocol server.
//!
//! `RynkService` is the transport-agnostic core. It holds a
//! [`KeyboardContext`](super::context::KeyboardContext) and exposes:
//!
//! - [`dispatch`](RynkService::dispatch) — process inbound message in-place.
//! - [`run_session`](RynkService::run_session) — drive one rynk session
//!   against a wire transport until it closes; emits topic frames between
//!   request/response turns.
//!
//! Topic handling lives in [`topics::TopicSubscribers`] which the session
//! drains; cache-only events (peripheral status) are mirrored into `ctx`
//! and not surfaced on the wire.

mod handlers;
pub(crate) mod topics;
// Shared error for RMK-authored adapters; only BLE hand-writes one today.
#[cfg(feature = "_ble")]
pub mod transport;
pub mod uart;

use embassy_futures::select::{Either, select};
use embedded_io_async::{Read, Write};
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{Cmd, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkMessage};
#[allow(unused_imports)] // re-exported at `crate::host` for downstream users
pub use uart::run_rynk_uart;

use super::context::KeyboardContext;
use crate::config::RmkConfig;
use crate::keymap::KeyMap;

const _: () = assert!(
    rmk_types::constants::RYNK_BUFFER_SIZE >= RYNK_MIN_BUFFER_SIZE,
    "rynk_buffer_size is smaller than RYNK_MIN_BUFFER_SIZE — set [rmk] \
     rynk_buffer_size in keyboard.toml, or disable features to shrink the \
     floor",
);

/// Maximum BLE chunk size that fits in a single GATT write
#[cfg(feature = "_ble")]
pub const RYNK_BLE_CHUNK_SIZE: usize = 244;

/// Transport-agnostic Rynk service.
pub struct RynkService<'a> {
    pub(super) ctx: KeyboardContext<'a>,
}

impl<'a> RynkService<'a> {
    pub fn new(keymap: &'a KeyMap<'a>, _config: &RmkConfig<'static>) -> Self {
        Self {
            ctx: KeyboardContext::new(keymap),
        }
    }

    /// Process one inbound message in place. Always writes a response
    /// envelope (Ok or Err) into `msg`; `cmd` and `seq` are echoed verbatim.
    pub async fn dispatch(&self, msg: &mut RynkMessage<'_>) {
        match self.handle(msg).await {
            Ok(n) => msg.set_payload_len(n as u16),
            Err(e) => msg.write_error(e),
        }
    }

    async fn handle(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let cmd = msg.cmd();
        let payload = msg.payload_mut();
        let payload_len = match cmd {
            // ── System ──
            Cmd::GetVersion => self.handle_get_version(payload).await?,
            Cmd::GetCapabilities => self.handle_get_capabilities(payload).await?,
            Cmd::Reboot => self.handle_reboot(payload).await?,
            Cmd::BootloaderJump => self.handle_bootloader_jump(payload).await?,
            Cmd::StorageReset => self.handle_storage_reset(payload).await?,

            // ── Keymap (incl. encoder) ──
            Cmd::GetKeyAction => self.handle_get_key_action(payload).await?,
            Cmd::SetKeyAction => self.handle_set_key_action(payload).await?,
            Cmd::GetDefaultLayer => self.handle_get_default_layer(payload).await?,
            Cmd::SetDefaultLayer => self.handle_set_default_layer(payload).await?,
            Cmd::GetEncoderAction => self.handle_get_encoder_action(payload).await?,
            Cmd::SetEncoderAction => self.handle_set_encoder_action(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetKeymapBulk => self.handle_get_keymap_bulk(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetKeymapBulk => self.handle_set_keymap_bulk(payload).await?,

            // ── Macro ──
            Cmd::GetMacro => self.handle_get_macro(payload).await?,
            Cmd::SetMacro => self.handle_set_macro(payload).await?,

            // ── Combo ──
            Cmd::GetCombo => self.handle_get_combo(payload).await?,
            Cmd::SetCombo => self.handle_set_combo(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetComboBulk => self.handle_get_combo_bulk(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetComboBulk => self.handle_set_combo_bulk(payload).await?,

            // ── Morse ──
            Cmd::GetMorse => self.handle_get_morse(payload).await?,
            Cmd::SetMorse => self.handle_set_morse(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::GetMorseBulk => self.handle_get_morse_bulk(payload).await?,
            #[cfg(feature = "bulk_transfer")]
            Cmd::SetMorseBulk => self.handle_set_morse_bulk(payload).await?,

            // ── Fork ──
            Cmd::GetFork => self.handle_get_fork(payload).await?,
            Cmd::SetFork => self.handle_set_fork(payload).await?,

            // ── Behavior ──
            Cmd::GetBehaviorConfig => self.handle_get_behavior_config(payload).await?,
            Cmd::SetBehaviorConfig => self.handle_set_behavior_config(payload).await?,

            // ── Connection ──
            Cmd::GetConnectionType => self.handle_get_connection_type(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::GetBleStatus => self.handle_get_ble_status(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::SwitchBleProfile => self.handle_switch_ble_profile(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => self.handle_clear_ble_profile(payload).await?,

            // ── Status ──
            Cmd::GetCurrentLayer => self.handle_get_current_layer(payload).await?,
            Cmd::GetMatrixState => self.handle_get_matrix_state(payload).await?,
            #[cfg(feature = "_ble")]
            Cmd::GetBatteryStatus => self.handle_get_battery_status(payload).await?,
            #[cfg(all(feature = "_ble", feature = "split"))]
            Cmd::GetPeripheralStatus => self.handle_get_peripheral_status(payload).await?,
            Cmd::GetWpm => self.handle_get_wpm(payload).await?,
            Cmd::GetSleepState => self.handle_get_sleep_state(payload).await?,
            Cmd::GetLedIndicator => self.handle_get_led_indicator(payload).await?,

            // Topic CMDs — host shouldn't be sending these as requests.
            Cmd::LayerChange | Cmd::WpmUpdate | Cmd::ConnectionChange | Cmd::SleepState | Cmd::LedIndicator => {
                return Err(RynkError::Invalid);
            }
            #[cfg(feature = "_ble")]
            Cmd::BatteryStatusTopic => return Err(RynkError::Invalid),
        };
        Ok(payload_len)
    }

    /// Encode `value` as the `Ok` arm of a `Result<T, RynkError>` envelope.
    pub(crate) fn write_response<T: serde::Serialize>(value: &T, payload: &mut [u8]) -> Result<usize, RynkError> {
        postcard::to_slice(&Ok::<&T, RynkError>(value), payload)
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)
    }
}

impl RynkService<'_> {
    /// Drive one rynk session against `rx`/`tx`. Returns when either half
    /// reports an error or EOF (USB endpoint disabled, BLE disconnect,
    /// UART read failure).
    ///
    /// Owns the reassembly/dispatch buffer, parses frames as `5 + LEN`
    /// headers, dispatches each frame in place via
    /// [`dispatch`](RynkService::dispatch), and emits topic frames between
    /// request/response turns. Transport-specific setup (USB
    /// `wait_connection`, BLE per-connection channel reset) and reconnect
    /// both stay in the caller — this returns on close rather than looping.
    pub async fn run_session<R: Read, T: Write>(&self, rx: &mut R, tx: &mut T) {
        let mut buf = [0u8; RYNK_BUFFER_SIZE];
        let mut topics = topics::TopicSubscribers::new();
        let mut rx_used = 0usize;

        loop {
            let recv_result = if rx_used == 0 {
                // Idle: race the wire against topic events. Topic emission
                // is only safe before frame accumulation starts — once
                // `rx_used > 0` the buffer is committed to that frame.
                match select(rx.read(&mut buf), topics.next_event(&self.ctx)).await {
                    Either::First(r) => r,
                    Either::Second(event) => {
                        match event.encode(&mut buf) {
                            Ok(msg) => {
                                let total = msg.frame_len();
                                if tx.write_all(&buf[..total]).await.is_err() {
                                    return;
                                }
                            }
                            Err(e) => warn!("Rynk topic encode failed: {:?}", e),
                        }
                        continue;
                    }
                }
            } else {
                let tail = &mut buf[rx_used..];
                if tail.is_empty() {
                    warn!("Rynk RX overflow; resyncing");
                    rx_used = 0;
                    continue;
                }
                rx.read(tail).await
            };

            match recv_result {
                Ok(0) => return, // EOF
                Ok(n) => rx_used += n,
                Err(_) => return,
            }

            // Drain every complete frame already buffered before reading
            // again. Without this, a host that pipelines two frames in one
            // syscall and then disconnects would lose the second frame —
            // the next read would return EOF before it could be processed.
            while rx_used >= RYNK_HEADER_SIZE {
                let payload_n = u16::from_le_bytes([buf[3], buf[4]]) as usize;
                let frame_len = RYNK_HEADER_SIZE + payload_n;
                if frame_len > buf.len() {
                    // Declared frame_len won't fit in the buffer.
                    warn!("Rynk: frame_len {} exceeds buffer {}", frame_len, buf.len());
                    let resp_len = RynkMessage::encode_error_reply(&mut buf, RynkError::Malformed);
                    if tx.write_all(&buf[..resp_len]).await.is_err() {
                        return;
                    }
                    rx_used = 0;
                    break;
                }
                if rx_used < frame_len {
                    break; // need more bytes
                }
                let resp_len = match RynkMessage::try_from(&mut buf[..frame_len]) {
                    Ok(mut msg) => {
                        self.dispatch(&mut msg).await;
                        msg.frame_len()
                    }
                    Err(e) => {
                        warn!("Rynk: invalid frame: {:?}", e);
                        RynkMessage::encode_error_reply(&mut buf, RynkError::Malformed)
                    }
                };
                if tx.write_all(&buf[..resp_len]).await.is_err() {
                    return;
                }
                // Preserve any pipelined trailing bytes for the next iteration.
                buf.copy_within(frame_len..rx_used, 0);
                rx_used -= frame_len;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::collections::VecDeque;
    use alloc::vec;
    use alloc::vec::Vec;

    use embedded_io_async::{ErrorKind, ErrorType, Read, Write};
    use rmk_types::action::KeyAction;
    use rmk_types::protocol::rynk::ProtocolVersion;

    use super::*;
    use crate::config::{BehaviorConfig, PositionalConfig, RmkConfig};
    use crate::keymap::{KeyMap, KeymapData};
    use crate::test_support::test_block_on as block_on;

    /// Returns each item in `chunks` as a separate `read` call, with partial
    /// buffers handled by draining bytes from the head of the front chunk.
    /// Yields `Ok(0)` (EOF) once all chunks are drained.
    struct ChunkRead {
        chunks: VecDeque<Vec<u8>>,
    }

    impl ErrorType for ChunkRead {
        type Error = ErrorKind;
    }

    impl Read for ChunkRead {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let Some(chunk) = self.chunks.front_mut() else {
                return Ok(0);
            };
            let n = chunk.len().min(buf.len());
            buf[..n].copy_from_slice(&chunk[..n]);
            chunk.drain(..n);
            if chunk.is_empty() {
                self.chunks.pop_front();
            }
            Ok(n)
        }
    }

    /// Captures every byte handed to `write` into a `Vec` for later assertion.
    struct VecWrite {
        captured: Vec<u8>,
    }

    impl ErrorType for VecWrite {
        type Error = ErrorKind;
    }

    impl Write for VecWrite {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.captured.extend_from_slice(buf);
            Ok(buf.len())
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    /// Build a `GetVersion` request frame with `seq`. The payload slot is sized
    /// to hold the in-place response (`Ok(ProtocolVersion)` = 3 bytes).
    fn get_version_frame(seq: u8) -> Vec<u8> {
        let cmd = Cmd::GetVersion as u16;
        let payload_len: u16 = 4;
        let total = RYNK_HEADER_SIZE + payload_len as usize;
        let mut v = vec![0u8; total];
        v[0..2].copy_from_slice(&cmd.to_le_bytes());
        v[2] = seq;
        v[3..5].copy_from_slice(&payload_len.to_le_bytes());
        v
    }

    /// Two pipelined `GetVersion` frames arriving across a split read — chunk 1
    /// carries all of frame 1 plus the first 3 bytes of frame 2 (header only),
    /// chunk 2 carries the rest of frame 2. The post-dispatch `copy_within`
    /// must preserve the in-flight prefix of frame 2; without it, `rx_used = 0`
    /// would discard those bytes and frame 2's response would never be emitted.
    #[test]
    fn run_session_preserves_pipelined_trailing_bytes() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        let frame_one = get_version_frame(0);
        let frame_two = get_version_frame(1);

        let mut chunk_a = frame_one.clone();
        chunk_a.extend_from_slice(&frame_two[..3]);
        let chunk_b = frame_two[3..].to_vec();

        let mut chunks = VecDeque::new();
        chunks.push_back(chunk_a);
        chunks.push_back(chunk_b);

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        // Response: 5-byte header + 3-byte `Ok(ProtocolVersion)` payload.
        const RESP_PAYLOAD_LEN: usize = 3;
        const RESP_FRAME_LEN: usize = RYNK_HEADER_SIZE + RESP_PAYLOAD_LEN;

        assert_eq!(
            tx.captured.len(),
            RESP_FRAME_LEN * 2,
            "expected two complete response frames; got {} bytes (would be {} without the pipelining fix)",
            tx.captured.len(),
            RESP_FRAME_LEN,
        );

        let mut expected_payload = [0u8; RESP_PAYLOAD_LEN];
        let n = postcard::to_slice(
            &Ok::<&ProtocolVersion, RynkError>(&ProtocolVersion::CURRENT),
            &mut expected_payload[..],
        )
        .unwrap()
        .len();
        assert_eq!(n, RESP_PAYLOAD_LEN);

        for (i, expected_seq) in [0u8, 1u8].iter().enumerate() {
            let off = i * RESP_FRAME_LEN;
            let resp = &tx.captured[off..off + RESP_FRAME_LEN];
            assert_eq!(
                &resp[0..2],
                &(Cmd::GetVersion as u16).to_le_bytes(),
                "response {i} cmd echo",
            );
            assert_eq!(resp[2], *expected_seq, "response {i} seq echo");
            assert_eq!(
                &resp[3..5],
                &(RESP_PAYLOAD_LEN as u16).to_le_bytes(),
                "response {i} payload_len",
            );
            assert_eq!(&resp[RYNK_HEADER_SIZE..], &expected_payload[..], "response {i} payload",);
        }
    }

    /// Two `GetVersion` frames delivered together in one `read` call, then EOF.
    /// Exercises the `while` drain after a successful read — without it, the
    /// post-frame-1 read would surface `Ok(0)` and return before frame 2 was
    /// dispatched.
    #[test]
    fn run_session_drains_pipelined_frames_before_eof() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        let mut combined = get_version_frame(0);
        combined.extend_from_slice(&get_version_frame(1));

        let mut chunks = VecDeque::new();
        chunks.push_back(combined);

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        const RESP_FRAME_LEN: usize = RYNK_HEADER_SIZE + 3;
        assert_eq!(
            tx.captured.len(),
            RESP_FRAME_LEN * 2,
            "expected both pipelined frames to be dispatched before EOF",
        );
        assert_eq!(tx.captured[2], 0, "first response seq");
        assert_eq!(tx.captured[RESP_FRAME_LEN + 2], 1, "second response seq");
    }
}
