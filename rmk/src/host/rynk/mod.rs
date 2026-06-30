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
//! drains and emits on the wire between request/response turns.

mod handlers;
pub(crate) mod topics;
pub mod uart;

use embassy_futures::select::{Either, select};
use embedded_io_async::{Read, Write};
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{
    Cmd, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkHeader, RynkMessage, command,
};
#[allow(unused_imports)] // re-exported at `crate::host` for downstream users
pub use uart::run_rynk_uart;

use self::handlers::Handle;
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

    /// Process one inbound message in place.
    /// Always writes a response envelope (Ok or Err) into `msg`.
    /// `cmd` and `seq` are echoed verbatim.
    pub async fn dispatch(&self, msg: &mut RynkMessage<'_>) {
        if let Err(e) = match msg.header().cmd {
            // System
            Cmd::GetVersion => Handle::<command::GetVersion>::handle_message(self, msg).await,
            Cmd::GetCapabilities => Handle::<command::GetCapabilities>::handle_message(self, msg).await,
            Cmd::Reboot => Handle::<command::Reboot>::handle_message(self, msg).await,
            Cmd::BootloaderJump => Handle::<command::BootloaderJump>::handle_message(self, msg).await,
            Cmd::StorageReset => Handle::<command::StorageReset>::handle_message(self, msg).await,

            // Keymap (incl. encoder)
            Cmd::GetKeyAction => Handle::<command::GetKeyAction>::handle_message(self, msg).await,
            Cmd::SetKeyAction => Handle::<command::SetKeyAction>::handle_message(self, msg).await,
            Cmd::GetDefaultLayer => Handle::<command::GetDefaultLayer>::handle_message(self, msg).await,
            Cmd::SetDefaultLayer => Handle::<command::SetDefaultLayer>::handle_message(self, msg).await,
            Cmd::GetEncoderAction => Handle::<command::GetEncoderAction>::handle_message(self, msg).await,
            Cmd::SetEncoderAction => Handle::<command::SetEncoderAction>::handle_message(self, msg).await,
            #[cfg(feature = "bulk")]
            Cmd::GetKeymapBulk => Handle::<command::GetKeymapBulk>::handle_message(self, msg).await,
            #[cfg(feature = "bulk")]
            Cmd::SetKeymapBulk => Handle::<command::SetKeymapBulk>::handle_message(self, msg).await,

            // Macro
            Cmd::GetMacro => Handle::<command::GetMacro>::handle_message(self, msg).await,
            Cmd::SetMacro => Handle::<command::SetMacro>::handle_message(self, msg).await,

            // Combo
            Cmd::GetCombo => Handle::<command::GetCombo>::handle_message(self, msg).await,
            Cmd::SetCombo => Handle::<command::SetCombo>::handle_message(self, msg).await,
            #[cfg(feature = "bulk")]
            Cmd::GetComboBulk => Handle::<command::GetComboBulk>::handle_message(self, msg).await,
            #[cfg(feature = "bulk")]
            Cmd::SetComboBulk => Handle::<command::SetComboBulk>::handle_message(self, msg).await,

            // Morse
            Cmd::GetMorse => Handle::<command::GetMorse>::handle_message(self, msg).await,
            Cmd::SetMorse => Handle::<command::SetMorse>::handle_message(self, msg).await,
            #[cfg(feature = "bulk")]
            Cmd::GetMorseBulk => Handle::<command::GetMorseBulk>::handle_message(self, msg).await,
            #[cfg(feature = "bulk")]
            Cmd::SetMorseBulk => Handle::<command::SetMorseBulk>::handle_message(self, msg).await,

            // Fork
            Cmd::GetFork => Handle::<command::GetFork>::handle_message(self, msg).await,
            Cmd::SetFork => Handle::<command::SetFork>::handle_message(self, msg).await,

            // Behavior
            Cmd::GetBehaviorConfig => Handle::<command::GetBehaviorConfig>::handle_message(self, msg).await,
            Cmd::SetBehaviorConfig => Handle::<command::SetBehaviorConfig>::handle_message(self, msg).await,

            // Connection
            Cmd::GetConnectionType => Handle::<command::GetConnectionType>::handle_message(self, msg).await,
            Cmd::GetConnectionStatus => Handle::<command::GetConnectionStatus>::handle_message(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBleStatus => Handle::<command::GetBleStatus>::handle_message(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::SwitchBleProfile => Handle::<command::SwitchBleProfile>::handle_message(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => Handle::<command::ClearBleProfile>::handle_message(self, msg).await,

            // Status
            Cmd::GetCurrentLayer => Handle::<command::GetCurrentLayer>::handle_message(self, msg).await,
            Cmd::GetMatrixState => Handle::<command::GetMatrixState>::handle_message(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBatteryStatus => Handle::<command::GetBatteryStatus>::handle_message(self, msg).await,
            #[cfg(all(feature = "_ble", feature = "split"))]
            Cmd::GetPeripheralStatus => Handle::<command::GetPeripheralStatus>::handle_message(self, msg).await,
            Cmd::GetWpm => Handle::<command::GetWpm>::handle_message(self, msg).await,
            Cmd::GetSleepState => Handle::<command::GetSleepState>::handle_message(self, msg).await,
            Cmd::GetLedIndicator => Handle::<command::GetLedIndicator>::handle_message(self, msg).await,

            // Topic CMDs are server→host push only. `run_session` drops
            // topic-range frames before dispatch; this arm is defense for
            // direct `dispatch` callers and unknown future topics.
            cmd if cmd.is_topic() => Err(RynkError::Invalid),
            _ => Err(RynkError::UnknownCmd),
        } {
            msg.encode_error(e);
        }
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

            // 2. Decode the header just read into buf[..RYNK_HEADER_SIZE].
            let Some(head) = buf.first_chunk() else { return };
            let header = RynkHeader::parse(head);
            let payload_n = header.payload_len as usize;
            let frame_len = header.frame_len();

            // 3. Drop non-dispatchable frames, draining the payload to resync
            // onto the next frame. Topic CMDs are push-only — a reply would be
            // re-queued by the host as a phantom topic — so drop them silently,
            // checked first so an oversized topic still draws no error.
            let is_topic = header.cmd.is_topic();
            if is_topic || frame_len > buf.len() {
                if is_topic {
                    warn!("Rynk: dropping topic-range request {:?}", header.cmd);
                } else {
                    warn!("Rynk: frame_len {} exceeds buffer {}", frame_len, buf.len());
                    // The declared payload is undeliverable; reply with a header-only
                    // error frame. `cmd`/`seq` in buf[..3] are preserved from the
                    // parsed header.
                    let n = postcard::to_slice(
                        &Err::<(), RynkError>(RynkError::Malformed),
                        &mut buf[RYNK_HEADER_SIZE..],
                    )
                    .map(|s| s.len())
                    .unwrap_or(0);
                    buf[3..5].copy_from_slice(&(n as u16).to_le_bytes());
                    if tx.write_all(&buf[..RYNK_HEADER_SIZE + n]).await.is_err() {
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

            // 5. Dispatch in place over the full buffer. `try_from` checks
            // only structural bounds (buffer covers header + declared LEN),
            // which steps 1–4 guarantee; whether the payload *decodes* is
            // dispatch's job and draws a Malformed reply, not a session end.
            let Ok(mut msg) = RynkMessage::try_from(&mut buf[..]) else {
                return;
            };

            self.dispatch(&mut msg).await;
            let resp_len = msg.frame_len();
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

    /// A bare 5-byte header with `cmd`, `seq`, and a declared `payload_len`.
    /// A `payload_len` of 0 is a complete empty-payload request (e.g. `GetVersion`).
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

        let frame_one = header(Cmd::GetVersion.raw(), 0, 0);
        let frame_two = header(Cmd::GetVersion.raw(), 1, 0);

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

        let mut combined = header(Cmd::GetVersion.raw(), 0, 0);
        combined.extend_from_slice(&header(Cmd::GetVersion.raw(), 1, 0));

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
        chunks.push_back(header(Cmd::GetVersion.raw(), 0x42, 0));

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
        combined.extend_from_slice(&header(Cmd::GetVersion.raw(), 7, 0));

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
