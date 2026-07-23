//! Rynk host service — RMK-native protocol server.
//!
//! `RynkService` owns the global keyboard state and dispatch policy. Each
//! transport run creates independent authorization and topic-subscription state.

mod handlers;
mod topics;
mod uart;

use embassy_futures::select::{Either, select};
use embedded_io_async::{Read, Write};
use rmk_types::protocol::rynk::{
    Cmd, Deframer, FirmwareVersion, RYNK_FRAME_BUFFER_SIZE, RynkError, RynkMessage, command,
};
#[allow(unused_imports)] // re-exported at `crate::host` for downstream users
pub use uart::run_rynk_uart;

use self::handlers::Serve;
use super::context::KeyboardContext;
use super::lock::HostLock;
use crate::config::{DeviceConfig, LockConfig, RmkConfig};
use crate::keymap::KeyMap;

/// Unlock attempts live long enough for BLE WebHID round trips.
const RYNK_UNLOCK_WINDOW: embassy_time::Duration = embassy_time::Duration::from_millis(500);

const RMK_VERSION: FirmwareVersion = {
    const fn component(s: &str) -> u8 {
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut value = 0u8;
        while i < bytes.len() {
            value = value * 10 + (bytes[i] - b'0');
            i += 1;
        }
        value
    }

    FirmwareVersion {
        major: component(env!("CARGO_PKG_VERSION_MAJOR")),
        minor: component(env!("CARGO_PKG_VERSION_MINOR")),
        patch: component(env!("CARGO_PKG_VERSION_PATCH")),
    }
};

/// Transport-agnostic Rynk service.
pub struct RynkService<'a> {
    ctx: KeyboardContext<'a>,
    /// Device identity served by `GetDeviceInfo`.
    device: DeviceConfig<'static>,
    /// Policy copied into each session's authorization gate.
    lock_config: LockConfig,
}

struct RynkSession<'a> {
    locker: HostLock<'a>,
    topics: topics::TopicSubscribers,
}

impl<'a> RynkService<'a> {
    pub fn new(keymap: &'a KeyMap<'a>, config: &RmkConfig<'static>) -> Self {
        let mut ctx = KeyboardContext::new(keymap);
        // Layout is fixed at macro expansion time, like Vial's keyboard-def.
        ctx.layout_blob = config.layout_blob;
        Self {
            ctx,
            device: config.device_config,
            lock_config: config.lock_config,
        }
    }

    /// Whether `cmd` needs an unlocked device.
    fn requires_unlock(&self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::BootloaderJump | Cmd::StorageReset | Cmd::GetMatrixState => true,
            // Deleting a bond opens a re-pair hijack window; BLE-only command.
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => true,
            Cmd::SetKeyAction
            | Cmd::SetDefaultLayer
            | Cmd::SetEncoderAction
            | Cmd::SetMacro
            | Cmd::SetCombo
            | Cmd::SetMorse
            | Cmd::SetFork
            | Cmd::SetBehaviorConfig
            | Cmd::SetKeymapBulk
            | Cmd::SetComboBulk
            | Cmd::SetMorseBulk => self.lock_config.write_requires_unlock,
            _ => false,
        }
    }

    /// Process one inbound message in place and replace its payload with a
    /// response envelope. `cmd` and `seq` remain unchanged.
    async fn dispatch(&self, session: &RynkSession<'_>, msg: &mut RynkMessage<'_>) {
        let cmd = msg.header().cmd;

        if self.requires_unlock(cmd) && !session.locker.is_unlocked() {
            msg.encode_error(RynkError::Locked);
            return;
        }

        if let Err(error) = match cmd {
            Cmd::GetVersion => Serve::<command::GetVersion, _>::serve(self, msg).await,
            Cmd::GetCapabilities => Serve::<command::GetCapabilities, _>::serve(self, msg).await,
            Cmd::Reboot => Serve::<command::Reboot, _>::serve(self, msg).await,
            Cmd::BootloaderJump => Serve::<command::BootloaderJump, _>::serve(self, msg).await,
            Cmd::StorageReset => Serve::<command::StorageReset, _>::serve(self, msg).await,
            Cmd::GetLockStatus => Serve::<command::GetLockStatus, _>::serve(session, msg).await,
            Cmd::UnlockPoll => Serve::<command::UnlockPoll, _>::serve(session, msg).await,
            Cmd::Lock => Serve::<command::Lock, _>::serve(session, msg).await,
            Cmd::GetDeviceInfo => Serve::<command::GetDeviceInfo, _>::serve(self, msg).await,

            Cmd::GetKeyAction => Serve::<command::GetKeyAction, _>::serve(self, msg).await,
            Cmd::SetKeyAction => Serve::<command::SetKeyAction, _>::serve(self, msg).await,
            Cmd::GetDefaultLayer => Serve::<command::GetDefaultLayer, _>::serve(self, msg).await,
            Cmd::SetDefaultLayer => Serve::<command::SetDefaultLayer, _>::serve(self, msg).await,
            Cmd::GetEncoderAction => Serve::<command::GetEncoderAction, _>::serve(self, msg).await,
            Cmd::SetEncoderAction => Serve::<command::SetEncoderAction, _>::serve(self, msg).await,
            Cmd::GetKeymapBulk => Serve::<command::GetKeymapBulk, _>::serve(self, msg).await,
            Cmd::SetKeymapBulk => Serve::<command::SetKeymapBulk, _>::serve(self, msg).await,

            Cmd::GetMacro => Serve::<command::GetMacro, _>::serve(self, msg).await,
            Cmd::SetMacro => Serve::<command::SetMacro, _>::serve(self, msg).await,

            Cmd::GetCombo => Serve::<command::GetCombo, _>::serve(self, msg).await,
            Cmd::SetCombo => Serve::<command::SetCombo, _>::serve(self, msg).await,
            Cmd::GetComboBulk => Serve::<command::GetComboBulk, _>::serve(self, msg).await,
            Cmd::SetComboBulk => Serve::<command::SetComboBulk, _>::serve(self, msg).await,
            Cmd::GetMorse => Serve::<command::GetMorse, _>::serve(self, msg).await,
            Cmd::SetMorse => Serve::<command::SetMorse, _>::serve(self, msg).await,
            Cmd::GetMorseBulk => Serve::<command::GetMorseBulk, _>::serve(self, msg).await,
            Cmd::SetMorseBulk => Serve::<command::SetMorseBulk, _>::serve(self, msg).await,

            Cmd::GetFork => Serve::<command::GetFork, _>::serve(self, msg).await,
            Cmd::SetFork => Serve::<command::SetFork, _>::serve(self, msg).await,

            Cmd::GetBehaviorConfig => Serve::<command::GetBehaviorConfig, _>::serve(self, msg).await,
            Cmd::SetBehaviorConfig => Serve::<command::SetBehaviorConfig, _>::serve(self, msg).await,

            Cmd::GetConnectionType => Serve::<command::GetConnectionType, _>::serve(self, msg).await,
            Cmd::GetConnectionStatus => Serve::<command::GetConnectionStatus, _>::serve(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBleStatus => Serve::<command::GetBleStatus, _>::serve(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::SwitchBleProfile => Serve::<command::SwitchBleProfile, _>::serve(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::ClearBleProfile => Serve::<command::ClearBleProfile, _>::serve(self, msg).await,

            Cmd::GetCurrentLayer => Serve::<command::GetCurrentLayer, _>::serve(self, msg).await,
            Cmd::GetMatrixState => Serve::<command::GetMatrixState, _>::serve(self, msg).await,
            #[cfg(feature = "_ble")]
            Cmd::GetBatteryStatus => Serve::<command::GetBatteryStatus, _>::serve(self, msg).await,
            #[cfg(feature = "split")]
            Cmd::GetPeripheralStatus => Serve::<command::GetPeripheralStatus, _>::serve(self, msg).await,
            Cmd::GetWpm => Serve::<command::GetWpm, _>::serve(self, msg).await,
            Cmd::GetSleepState => Serve::<command::GetSleepState, _>::serve(self, msg).await,
            Cmd::GetLedIndicator => Serve::<command::GetLedIndicator, _>::serve(self, msg).await,

            Cmd::GetLayout => Serve::<command::GetLayout, _>::serve(self, msg).await,

            _ => Err(RynkError::UnknownCmd),
        } {
            msg.encode_error(error);
        }
    }

    /// Drive one rynk session based on embedded-io `rx`/`tx`.
    ///
    /// Owns frame reassembly/dispatch; transport setup and reconnect stay outside.
    pub async fn run_session<R: Read, T: Write>(&self, rx: &mut R, tx: &mut T) {
        let mut session = RynkSession {
            locker: HostLock::new(
                self.lock_config.unlock_keys,
                self.ctx.keymap,
                self.lock_config.insecure,
                RYNK_UNLOCK_WINDOW,
            ),
            topics: topics::TopicSubscribers::new(),
        };
        let mut buf = [0u8; RYNK_FRAME_BUFFER_SIZE];
        let mut df = Deframer::new();
        // Mute topics until the client completes the version handshake.
        let mut handshaked = false;

        loop {
            // Dispatch one complete frame in place, then discard the rest of the
            // buffer. RMK's client is one-in-flight, so a read holds at most one
            // request; anything trailing it — a batching host's next frame, or a
            // BLE-HID report's zero padding — is dropped, which lets the reply
            // grow into the whole buffer instead of reserving space for it.
            if let Some(frame_len) = df.next(&mut buf) {
                let mut msg = RynkMessage::from_decoded(&mut buf, frame_len);
                let cmd = msg.header().cmd;
                if cmd.is_topic() {
                    // Hosts never send topic-range cmds; drop without a reply.
                    warn!("Rynk: dropping topic-range request {:?}", cmd);
                } else {
                    self.dispatch(&session, &mut msg).await;
                    // The version handshake completes on GetCapabilities.
                    handshaked |= cmd == Cmd::GetCapabilities;
                    if tx.write_all(msg.frame()).await.is_err() {
                        return;
                    }
                }
                df = Deframer::new();
            }

            // No complete frame buffered. A half-received frame must be finished by
            // reading only (a topic emit would clobber it); with an empty buffer,
            // race a read against the next topic to forward.
            if df.has_pending() {
                match rx.read(df.tail(&mut buf)).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => df.commit(n),
                }
            } else {
                match select(rx.read(df.tail(&mut buf)), session.topics.next_event()).await {
                    Either::First(Ok(0)) | Either::First(Err(_)) => return,
                    Either::First(Ok(n)) => df.commit(n),
                    Either::Second(event) => {
                        // Not handshaked yet: drain the event but don't forward it.
                        if !handshaked {
                            continue;
                        }
                        // The buffer is empty here, so encoding the topic is safe.
                        match event.encode(&mut buf) {
                            Ok(msg) => {
                                if tx.write_all(msg.frame()).await.is_err() {
                                    return;
                                }
                            }
                            Err(e) => warn!("Rynk topic encode failed: {:?}", e),
                        }
                    }
                }
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

    use embassy_futures::join::join;
    use embedded_io_async::{ErrorKind, ErrorType, Read, Write};
    use rmk_types::action::KeyAction;
    use rmk_types::protocol::rynk::{LockStatus, MatrixState, ProtocolVersion, RYNK_HEADER_SIZE, RynkHeader};

    use super::*;
    use crate::config::{BehaviorConfig, LockConfig, PositionalConfig, RmkConfig};
    use crate::event::KeyboardEvent;
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

    /// A COBS request frame with an empty payload, as a host sends it.
    fn req(cmd_raw: u16, seq: u8) -> Vec<u8> {
        let mut buf = [0u8; 32];
        RynkMessage::build(
            &mut buf,
            RynkHeader {
                cmd: Cmd::from_raw(cmd_raw),
                seq,
            },
            &(),
        )
        .unwrap()
        .frame()
        .to_vec()
    }

    /// Decode a captured response stream into `(cmd, seq, decoded payload)` per
    /// frame, using the production Deframer.
    fn decode_frames(buf: &[u8]) -> Vec<(u16, u8, Vec<u8>)> {
        let mut work = buf.to_vec();
        let mut df = Deframer::new();
        df.commit(work.len());
        let mut out = Vec::new();
        loop {
            let Some(frame_len) = df.next(&mut work) else { break };
            let frame = &work[..frame_len];
            out.push((
                u16::from_le_bytes([frame[0], frame[1]]),
                frame[2],
                frame[RYNK_HEADER_SIZE..].to_vec(),
            ));
        }
        out
    }

    /// Lock gate over `run_session`, including fresh authorization per session.
    #[test]
    fn run_session_lock_gate_and_new_session_starts_locked() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<2, 2> = PositionalConfig::default();
        let mut data: KeymapData<2, 2, 1, 0> =
            KeymapData::new([[[KeyAction::No, KeyAction::No], [KeyAction::No, KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));

        const UNLOCK_KEYS: &[(u8, u8)] = &[(0, 0)];
        let mut config = RmkConfig::default();
        config.lock_config = LockConfig {
            unlock_keys: UNLOCK_KEYS,
            insecure: false,
            write_requires_unlock: false,
        };
        let service = RynkService::new(&keymap, &config);

        // Hold the challenge key for the whole session.
        keymap.update_matrix_state(&KeyboardEvent::key(0, 0, true));

        // Locked probe, status, unlock poll, unlocked probe — one frame per read,
        // as the one-in-flight client sends them.
        let mut chunks = VecDeque::new();
        chunks.push_back(req(Cmd::GetMatrixState.raw(), 0));
        chunks.push_back(req(Cmd::GetLockStatus.raw(), 1));
        chunks.push_back(req(Cmd::UnlockPoll.raw(), 2));
        chunks.push_back(req(Cmd::GetMatrixState.raw(), 3));
        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };
        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(resp.len(), 4, "one reply per request");

        // Locked matrix reads reject instead of returning an empty bitmap.
        assert_eq!(resp[0].0, Cmd::GetMatrixState.raw());
        assert_eq!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(&resp[0].2).unwrap(),
            Err(RynkError::Locked),
            "keystroke exfiltration is gated"
        );

        // Lock status is open while locked.
        let status: LockStatus = postcard::from_bytes::<Result<LockStatus, RynkError>>(&resp[1].2)
            .unwrap()
            .unwrap();
        assert!(status.locked);
        assert_eq!(
            status.key_positions.as_slice(),
            &[(0, 0)],
            "challenge advertised while locked"
        );

        // Held challenge key unlocks.
        let polled: LockStatus = postcard::from_bytes::<Result<LockStatus, RynkError>>(&resp[2].2)
            .unwrap()
            .unwrap();
        assert!(!polled.locked, "poll with challenge key held unlocks");
        assert_eq!(polled.remaining_keys, 0);

        // Gated command succeeds after unlock.
        assert!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(&resp[3].2)
                .unwrap()
                .is_ok(),
            "gated command served once unlocked"
        );

        // New session starts locked again.
        let mut chunks2 = VecDeque::new();
        chunks2.push_back(req(Cmd::GetMatrixState.raw(), 0));
        let mut rx2 = ChunkRead { chunks: chunks2 };
        let mut tx2 = VecWrite { captured: Vec::new() };
        block_on(service.run_session(&mut rx2, &mut tx2));

        let resp2 = decode_frames(&tx2.captured);
        assert_eq!(resp2.len(), 1);
        assert_eq!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(&resp2[0].2).unwrap(),
            Err(RynkError::Locked),
            "a fresh session has independent locked state"
        );
    }

    #[test]
    fn sessions_authorize_independently() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<2, 2> = PositionalConfig::default();
        let mut data: KeymapData<2, 2, 1, 0> =
            KeymapData::new([[[KeyAction::No, KeyAction::No], [KeyAction::No, KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));

        const UNLOCK_KEYS: &[(u8, u8)] = &[(0, 0)];
        let mut config = RmkConfig::default();
        config.lock_config.unlock_keys = UNLOCK_KEYS;
        let service = RynkService::new(&keymap, &config);
        keymap.update_matrix_state(&KeyboardEvent::key(0, 0, true));

        let mut chunks_a = VecDeque::new();
        chunks_a.push_back(req(Cmd::UnlockPoll.raw(), 0x11));
        chunks_a.push_back(req(Cmd::GetLockStatus.raw(), 0x12));
        chunks_a.push_back(req(Cmd::Lock.raw(), 0x13));
        chunks_a.push_back(req(Cmd::GetMatrixState.raw(), 0x14));
        let mut rx_a = ChunkRead { chunks: chunks_a };
        let mut tx_a = VecWrite { captured: Vec::new() };

        let mut chunks_b = VecDeque::new();
        chunks_b.push_back(req(Cmd::GetLockStatus.raw(), 0x21));
        chunks_b.push_back(req(Cmd::UnlockPoll.raw(), 0x22));
        chunks_b.push_back(req(Cmd::GetMatrixState.raw(), 0x23));
        let mut rx_b = ChunkRead { chunks: chunks_b };
        let mut tx_b = VecWrite { captured: Vec::new() };

        block_on(join(
            service.run_session(&mut rx_a, &mut tx_a),
            service.run_session(&mut rx_b, &mut tx_b),
        ));

        let responses_a = decode_frames(&tx_a.captured);
        assert_eq!(responses_a.len(), 4);
        let unlocked_a = postcard::from_bytes::<Result<LockStatus, RynkError>>(&responses_a[0].2)
            .unwrap()
            .unwrap();
        assert!(!unlocked_a.locked);
        let status_a = postcard::from_bytes::<Result<LockStatus, RynkError>>(&responses_a[1].2)
            .unwrap()
            .unwrap();
        assert!(!status_a.locked);
        assert_eq!(
            postcard::from_bytes::<Result<(), RynkError>>(&responses_a[2].2).unwrap(),
            Ok(())
        );
        assert_eq!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(&responses_a[3].2).unwrap(),
            Err(RynkError::Locked),
        );

        let responses_b = decode_frames(&tx_b.captured);
        assert_eq!(responses_b.len(), 3);
        let locked_b = postcard::from_bytes::<Result<LockStatus, RynkError>>(&responses_b[0].2)
            .unwrap()
            .unwrap();
        assert!(locked_b.locked, "session A does not unlock session B");
        let unlocked_b = postcard::from_bytes::<Result<LockStatus, RynkError>>(&responses_b[1].2)
            .unwrap()
            .unwrap();
        assert!(!unlocked_b.locked);
        assert!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(&responses_b[2].2)
                .unwrap()
                .is_ok(),
            "locking session A does not relock session B"
        );
    }

    #[test]
    fn matrix_state_uses_rynk_column_order() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<2, 14> = PositionalConfig::default();
        let mut data: KeymapData<2, 14, 1, 0> = KeymapData::new([[[KeyAction::No; 14]; 2]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));

        let mut config = RmkConfig::default();
        config.lock_config.insecure = true;
        let service = RynkService::new(&keymap, &config);

        keymap.update_matrix_state(&KeyboardEvent::key(0, 0, true));
        keymap.update_matrix_state(&KeyboardEvent::key(0, 9, true));
        keymap.update_matrix_state(&KeyboardEvent::key(1, 6, true));
        keymap.update_matrix_state(&KeyboardEvent::key(1, 13, true));

        let mut chunks = VecDeque::new();
        chunks.push_back(req(Cmd::GetMatrixState.raw(), 0));
        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };
        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(resp.len(), 1);
        let state: MatrixState = postcard::from_bytes::<Result<MatrixState, RynkError>>(&resp[0].2)
            .unwrap()
            .unwrap();
        assert_eq!(&state.pressed_bitmap[..4], &[0x01, 0x02, 0x40, 0x20]);
        assert!(state.pressed_bitmap[4..].iter().all(|&b| b == 0));
    }

    /// A read carrying more than one frame (a batching host — which RMK's
    /// one-in-flight client never is) serves the first and discards the rest:
    /// the session stays alive and uncorrupted instead of clobbering a reply.
    #[test]
    fn run_session_serves_first_frame_of_a_coalesced_read() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // Two frames in one read; the first is served, the coalesced tail dropped.
        let mut combined = req(Cmd::GetVersion.raw(), 0);
        combined.extend_from_slice(&req(Cmd::GetVersion.raw(), 1));

        let mut chunks = VecDeque::new();
        chunks.push_back(combined);

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(resp.len(), 1, "first frame served; the coalesced tail is dropped");
        assert_eq!(resp[0].1, 0, "the reply is for the first frame");
        assert_eq!(
            postcard::from_bytes::<Result<ProtocolVersion, RynkError>>(&resp[0].2).unwrap(),
            Ok(ProtocolVersion::CURRENT),
        );
    }

    /// Zero-payload requests must still get a full response payload.
    #[test]
    fn run_session_empty_request_gets_full_response() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        let mut chunks = VecDeque::new();
        chunks.push_back(req(Cmd::GetVersion.raw(), 0x42));

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(resp.len(), 1);
        assert_eq!(resp[0].0, Cmd::GetVersion.raw(), "cmd echo");
        assert_eq!(resp[0].1, 0x42, "seq echo");
        assert!(
            !resp[0].2.is_empty(),
            "response carries a payload, not a swallowed fault"
        );
        let decoded: Result<ProtocolVersion, RynkError> =
            postcard::from_bytes(&resp[0].2).expect("response payload must decode");
        assert_eq!(decoded, Ok(ProtocolVersion::CURRENT));
    }

    /// Topic-range requests are drained without creating phantom topic replies.
    #[test]
    fn run_session_drops_topic_range_request_without_reply() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // A topic-range request, then a real request — one frame per read.
        let mut chunks = VecDeque::new();
        chunks.push_back(req(Cmd::LayerChange.raw(), 0));
        chunks.push_back(req(Cmd::GetVersion.raw(), 7));

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(
            resp.len(),
            1,
            "topic-range request draws no reply; only GetVersion answers"
        );
        assert_eq!(resp[0].0, Cmd::GetVersion.raw(), "cmd echo");
        assert_eq!(resp[0].1, 7, "reply is for the GetVersion that followed");
    }

    /// A delimiter-less run larger than the buffer overflows the Deframer and is
    /// dropped: no reply, and the session keeps running.
    #[test]
    fn run_session_oversized_frame_draws_no_reply() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // Non-zero bytes with no delimiter, longer than the frame buffer.
        let mut chunks = VecDeque::new();
        chunks.push_back(vec![0xFFu8; RYNK_FRAME_BUFFER_SIZE + 100]);

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        assert!(
            tx.captured.is_empty(),
            "oversized garbage draws no reply, got {} bytes",
            tx.captured.len()
        );
    }

    /// Garbage on the wire is skipped; the session resyncs and answers the next
    /// well-formed request (no cmd/seq to recover, so no Malformed echo).
    #[test]
    fn run_session_resyncs_after_garbage() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // Undecodable bytes terminated by a delimiter, then a real request.
        let mut stream = vec![0xDEu8, 0xAD, 0xBE, 0xEF, 0x00];
        stream.extend_from_slice(&req(Cmd::GetVersion.raw(), 0x55));
        let mut chunks = VecDeque::new();
        chunks.push_back(stream);

        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };

        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(resp.len(), 1, "garbage skipped, request answered");
        assert_eq!(resp[0].0, Cmd::GetVersion.raw());
        assert_eq!(resp[0].1, 0x55, "seq echoed after resync");
    }
}
