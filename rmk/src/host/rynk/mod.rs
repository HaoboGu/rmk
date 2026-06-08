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
//! Topic handling lives in [`topics::TopicSubscribers`], which the session
//! drains and emits on the wire between request/response turns — these topics
//! are the only server→host pushes. Snapshot state such as peripheral status
//! is not a topic; it is pull-only via its `Get*` handler.

mod handlers;
pub(crate) mod topics;
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

// Use `core::assert!` explicitly: in a `defmt` build the crate-level `assert!`
// expands to `defmt::assert!`, whose panic path is not `const`-callable.
const _: () = core::assert!(
    rmk_types::constants::RYNK_BUFFER_SIZE >= RYNK_MIN_BUFFER_SIZE,
    "rynk_buffer_size is smaller than RYNK_MIN_BUFFER_SIZE — set [rmk] \
     rynk_buffer_size in keyboard.toml, or disable features to shrink the \
     floor",
);

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
        // Each handler decodes its request via `msg.request::<T>()` (bounded by
        // the declared LEN) and writes its response to `msg.response_payload_mut()`.
        let payload_len = match msg.cmd() {
            // System
            Cmd::GetVersion => self.handle_get_version(msg).await?,
            Cmd::GetCapabilities => self.handle_get_capabilities(msg).await?,
            Cmd::Reboot => self.handle_reboot(msg).await?,
            Cmd::BootloaderJump => self.handle_bootloader_jump(msg).await?,
            Cmd::StorageReset => self.handle_storage_reset(msg).await?,

            // Keymap (incl. encoder)
            Cmd::GetKeyAction => self.handle_get_key_action(msg).await?,
            Cmd::SetKeyAction => self.handle_set_key_action(msg).await?,
            Cmd::GetDefaultLayer => self.handle_get_default_layer(msg).await?,
            Cmd::SetDefaultLayer => self.handle_set_default_layer(msg).await?,
            Cmd::GetEncoderAction => self.handle_get_encoder_action(msg).await?,
            Cmd::SetEncoderAction => self.handle_set_encoder_action(msg).await?,
            #[cfg(feature = "bulk")]
            Cmd::GetKeymapBulk => self.handle_get_keymap_bulk(msg).await?,
            #[cfg(feature = "bulk")]
            Cmd::SetKeymapBulk => self.handle_set_keymap_bulk(msg).await?,

            // Macro
            Cmd::GetMacro => self.handle_get_macro(msg).await?,
            Cmd::SetMacro => self.handle_set_macro(msg).await?,

            // Combo
            Cmd::GetCombo => self.handle_get_combo(msg).await?,
            Cmd::SetCombo => self.handle_set_combo(msg).await?,
            #[cfg(feature = "bulk")]
            Cmd::GetComboBulk => self.handle_get_combo_bulk(msg).await?,
            #[cfg(feature = "bulk")]
            Cmd::SetComboBulk => self.handle_set_combo_bulk(msg).await?,

            // Morse
            Cmd::GetMorse => self.handle_get_morse(msg).await?,
            Cmd::SetMorse => self.handle_set_morse(msg).await?,
            #[cfg(feature = "bulk")]
            Cmd::GetMorseBulk => self.handle_get_morse_bulk(msg).await?,
            #[cfg(feature = "bulk")]
            Cmd::SetMorseBulk => self.handle_set_morse_bulk(msg).await?,

            // Fork
            Cmd::GetFork => self.handle_get_fork(msg).await?,
            Cmd::SetFork => self.handle_set_fork(msg).await?,

            // Behavior
            Cmd::GetBehaviorConfig => self.handle_get_behavior_config(msg).await?,
            Cmd::SetBehaviorConfig => self.handle_set_behavior_config(msg).await?,

            // Connection
            Cmd::GetConnectionType => self.handle_get_connection_type(msg).await?,
            Cmd::GetConnectionStatus => self.handle_get_connection_status(msg).await?,
            #[cfg(feature = "_ble")]
            Cmd::GetBleStatus => self.handle_get_ble_status(msg).await?,
            #[cfg(feature = "_ble")]
            Cmd::SwitchBleProfile => self.handle_switch_ble_profile(msg).await?,
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => self.handle_clear_ble_profile(msg).await?,

            // Status
            Cmd::GetCurrentLayer => self.handle_get_current_layer(msg).await?,
            Cmd::GetMatrixState => self.handle_get_matrix_state(msg).await?,
            #[cfg(feature = "_ble")]
            Cmd::GetBatteryStatus => self.handle_get_battery_status(msg).await?,
            #[cfg(all(feature = "_ble", feature = "split"))]
            Cmd::GetPeripheralStatus => self.handle_get_peripheral_status(msg).await?,
            Cmd::GetWpm => self.handle_get_wpm(msg).await?,
            Cmd::GetSleepState => self.handle_get_sleep_state(msg).await?,
            Cmd::GetLedIndicator => self.handle_get_led_indicator(msg).await?,

            // Topic CMDs are server→host push only. `run_session` drops
            // topic-range frames before dispatch; this arm is defense for
            // direct `dispatch` callers and unknown future topics.
            cmd if cmd.is_topic() => return Err(RynkError::Invalid),
            _ => return Err(RynkError::UnknownCmd),
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
    /// Drive one rynk session based on embedded-io `rx`/`tx`.
    ///
    /// Owns the reassembly/dispatch buffer, parses frames as `5 + LEN`
    /// headers, dispatches each frame in place via
    /// [`dispatch`](RynkService::dispatch), and emits topic frames between
    /// request/response turns.
    ///
    /// Transport-specific setup and reconnect both stay in the caller.
    pub async fn run_session<R: Read, T: Write>(&self, rx: &mut R, tx: &mut T) {
        let mut buf = [0u8; RYNK_BUFFER_SIZE];
        let mut topics = topics::TopicSubscribers::new();

        loop {
            // 1. Read the fixed header or a topic
            match select(rx.read(&mut buf[..RYNK_HEADER_SIZE]), topics.next_event()).await {
                Either::First(r) => match r {
                    Ok(0) => return, // EOF
                    Ok(n) => {
                        if n < RYNK_HEADER_SIZE && rx.read_exact(&mut buf[n..RYNK_HEADER_SIZE]).await.is_err() {
                            // Error when reading header
                            return;
                        }
                    }
                    Err(_) => return,
                },
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
            };

            // 2. Read the declared payload length (LEN u16 LE) and CMD.
            let payload_n = u16::from_le_bytes([buf[3], buf[4]]) as usize;
            let frame_len = RYNK_HEADER_SIZE + payload_n;
            let cmd = Cmd::from_le_bytes([buf[0], buf[1]]);

            // 3. Drop non-dispatchable frames, draining the declared payload to
            // resync the stream. Topic-range CMDs are push-only and draw no
            // reply — a high-bit error frame would be queued by the host as a
            // phantom topic; oversized frames reply Malformed (unless also a
            // topic, checked here so the high-bit case never gets an error).
            if cmd.is_topic() || frame_len > buf.len() {
                if cmd.is_topic() {
                    warn!("Rynk: dropping topic-range request {:?}", cmd);
                } else {
                    warn!("Rynk: frame_len {} exceeds buffer {}", frame_len, buf.len());
                    let resp_len = RynkMessage::encode_error_reply(&mut buf, RynkError::Malformed);
                    if tx.write_all(&buf[..resp_len]).await.is_err() {
                        return;
                    }
                }
                let mut remaining = payload_n;
                while remaining > 0 {
                    let take = remaining.min(buf.len());
                    match rx.read(&mut buf[..take]).await {
                        Ok(0) => return,
                        Ok(n) => remaining -= n,
                        Err(_) => return,
                    }
                }
                continue;
            }

            // 4. Read exactly the payload.
            if rx.read_exact(&mut buf[RYNK_HEADER_SIZE..frame_len]).await.is_err() {
                return;
            }

            // 5. Dispatch in place over the full buffer.
            let resp_len = match RynkMessage::try_from(&mut buf[..]) {
                Ok(mut msg) => {
                    self.dispatch(&mut msg).await;
                    msg.frame_len()
                }
                Err(e) => {
                    // Steps 1–3 should guarantee the buffer holds the whole
                    // frame. If validation still fails, echo the structural
                    // error with cmd/seq preserved.
                    warn!("Rynk: invalid frame: {:?}", e);
                    RynkMessage::encode_error_reply(&mut buf, e)
                }
            };
            if tx.write_all(&buf[..resp_len]).await.is_err() {
                return;
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

    /// Build a `GetVersion` request frame with `seq`. The request carries an
    /// empty payload — the response is written into the full session buffer,
    /// not this slot, so no padding is needed.
    fn get_version_frame(seq: u8) -> Vec<u8> {
        let cmd = Cmd::GetVersion.raw();
        let payload_len: u16 = 0;
        let total = RYNK_HEADER_SIZE + payload_len as usize;
        let mut v = vec![0u8; total];
        v[0..2].copy_from_slice(&cmd.to_le_bytes());
        v[2] = seq;
        v[3..5].copy_from_slice(&payload_len.to_le_bytes());
        v
    }

    /// A bare 5-byte header with `cmd`, `seq`, and a declared `payload_len`.
    fn header(cmd_raw: u16, seq: u8, payload_len: u16) -> Vec<u8> {
        let mut v = vec![0u8; RYNK_HEADER_SIZE];
        v[0..2].copy_from_slice(&cmd_raw.to_le_bytes());
        v[2] = seq;
        v[3..5].copy_from_slice(&payload_len.to_le_bytes());
        v
    }

    /// Two pipelined `GetVersion` frames arriving across a split read — chunk 1
    /// carries all of frame 1 plus the first 3 bytes of frame 2 (header only),
    /// chunk 2 carries the rest of frame 2. Framed reads size each `read` to the
    /// current frame, so frame 2's in-flight bytes stay in the transport between
    /// iterations and both responses are emitted.
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
            assert_eq!(&resp[0..2], &Cmd::GetVersion.to_le_bytes(), "response {i} cmd echo",);
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
    /// Framed reads consume frame 1 first, leaving frame 2's bytes in the
    /// transport for the next iteration, so both are dispatched before EOF.
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

    /// Regression: a `GetVersion` request with `payload_len = 0` (the natural
    /// host request — `GetVersion` has no arguments) must produce a fully
    /// decodable `Ok(ProtocolVersion)` reply. Previously the response was
    /// squeezed into the 0-byte request slot, failed to encode, and was
    /// silently swallowed into a header-only `[01 00 00 00 00]` frame the host
    /// would misread as an empty success.
    #[test]
    fn run_session_empty_request_gets_full_response() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        let mut chunks = VecDeque::new();
        chunks.push_back(get_version_frame(0x42));

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        let resp = &tx.captured;
        assert!(
            resp.len() > RYNK_HEADER_SIZE,
            "response must carry a payload, not just a header"
        );
        assert_eq!(&resp[0..2], &Cmd::GetVersion.to_le_bytes(), "cmd echo");
        assert_eq!(resp[2], 0x42, "seq echo");

        let payload_len = u16::from_le_bytes([resp[3], resp[4]]) as usize;
        assert!(payload_len > 0, "payload_len must be non-zero (not a swallowed fault)");
        assert_eq!(
            resp.len(),
            RYNK_HEADER_SIZE + payload_len,
            "frame length matches header"
        );

        let decoded: Result<ProtocolVersion, RynkError> =
            postcard::from_bytes(&resp[RYNK_HEADER_SIZE..]).expect("response payload must decode");
        assert_eq!(decoded, Ok(ProtocolVersion::CURRENT));
    }

    /// A topic-range CMD arriving as a request is dropped without a reply — a
    /// high-bit error frame would be queued by the host as a phantom topic. Its
    /// payload is drained, so the session resyncs and answers the next request.
    #[test]
    fn run_session_drops_topic_range_request_without_reply() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // Topic-range request: LayerChange with a 1-byte payload, then a real
        // GetVersion — both in one chunk.
        let mut combined = header(Cmd::LayerChange.raw(), 0, 1);
        combined.push(0xAB);
        combined.extend_from_slice(&get_version_frame(7));

        let mut chunks = VecDeque::new();
        chunks.push_back(combined);

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        const RESP_FRAME_LEN: usize = RYNK_HEADER_SIZE + 3;
        assert_eq!(
            tx.captured.len(),
            RESP_FRAME_LEN,
            "topic-range request must draw no reply; only the GetVersion answers"
        );
        assert_eq!(&tx.captured[0..2], &Cmd::GetVersion.to_le_bytes(), "cmd echo");
        assert_eq!(tx.captured[2], 7, "reply is for the GetVersion that followed");
    }

    /// Regression for the topic/oversize ordering: an oversized declared LEN on
    /// a topic-range CMD must still draw no reply. Before the fix the oversize
    /// branch ran first and echoed a high-bit error frame.
    #[test]
    fn run_session_oversized_topic_frame_draws_no_reply() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // LayerChange topic with LEN = u16::MAX (far past the session buffer);
        // no payload follows, so the drain hits EOF and the session ends.
        let mut chunks = VecDeque::new();
        chunks.push_back(header(Cmd::LayerChange.raw(), 0, u16::MAX));

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        assert!(
            tx.captured.is_empty(),
            "oversized topic-range frame must draw no reply, got {} bytes",
            tx.captured.len()
        );
    }

    /// The non-topic half of the same branch: an oversized declared LEN on a
    /// normal request still draws a `Malformed` reply with cmd/seq preserved.
    #[test]
    fn run_session_oversized_request_replies_malformed() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // GetVersion with LEN = u16::MAX; no payload follows, drain hits EOF.
        let mut chunks = VecDeque::new();
        chunks.push_back(header(Cmd::GetVersion.raw(), 0x55, u16::MAX));

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        assert!(!tx.captured.is_empty(), "oversized request must draw a Malformed reply");
        assert_eq!(&tx.captured[0..2], &Cmd::GetVersion.to_le_bytes(), "cmd echo");
        assert_eq!(tx.captured[2], 0x55, "seq echo");
        let payload_len = u16::from_le_bytes([tx.captured[3], tx.captured[4]]) as usize;
        let decoded: Result<(), RynkError> =
            postcard::from_bytes(&tx.captured[RYNK_HEADER_SIZE..RYNK_HEADER_SIZE + payload_len])
                .expect("error reply must decode");
        assert_eq!(decoded, Err(RynkError::Malformed));
    }
}
