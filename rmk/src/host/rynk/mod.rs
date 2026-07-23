//! Rynk host service — RMK-native protocol server.
//!
//! `RynkService` owns the global keyboard state and dispatch policy. Each
//! transport run creates independent authorization and topic-subscription state.

mod handlers;
mod topics;
mod uart;

use embassy_futures::select::{Either, select};
use embedded_io_async::{Read, Write};
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{Cmd, FirmwareVersion, RYNK_HEADER_SIZE, RynkError, RynkHeader, RynkMessage, command};
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
        let mut buf = [0u8; RYNK_BUFFER_SIZE];
        // Mute topics until the client completes the version handshake.
        let mut handshaked = false;

        loop {
            // Read either a request header or the next outgoing topic.
            match select(rx.read(&mut buf[..RYNK_HEADER_SIZE]), session.topics.next_event()).await {
                Either::First(r) => match r {
                    Ok(0) => return, // EOF
                    Ok(n) => {
                        if n < RYNK_HEADER_SIZE && rx.read_exact(&mut buf[n..RYNK_HEADER_SIZE]).await.is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                },
                Either::Second(event) => {
                    // Not handshaked yet: drain the event but don't forward it.
                    if !handshaked {
                        continue;
                    }
                    match event.encode(&mut buf) {
                        Ok(msg) => {
                            if tx.write_all(msg.frame()).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => warn!("Rynk topic encode failed: {:?}", e),
                    }
                    continue;
                }
            };

            let Some(head) = buf.first_chunk() else { return };
            let header = RynkHeader::parse(head);
            let payload_n = header.payload_len as usize;
            let frame_len = header.frame_len();

            // Drain non-dispatchable payloads to resync. Topics get no reply.
            let is_topic = header.cmd.is_topic();
            if is_topic || frame_len > buf.len() {
                if is_topic {
                    warn!("Rynk: dropping topic-range request {:?}", header.cmd);
                } else {
                    warn!("Rynk: frame_len {} exceeds buffer {}", frame_len, buf.len());
                    // Echo cmd/seq back with a Malformed error; the payload was never read.
                    let err = Err::<(), RynkError>(RynkError::Malformed);
                    if let Ok(msg) = RynkMessage::build(&mut buf[..], header.cmd, header.seq, &err)
                        && tx.write_all(msg.frame()).await.is_err()
                    {
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

            if rx.read_exact(&mut buf[RYNK_HEADER_SIZE..frame_len]).await.is_err() {
                return;
            }

            let Ok(mut msg) = RynkMessage::try_from(&mut buf[..]) else {
                return;
            };

            self.dispatch(&session, &mut msg).await;
            // The version has been negotiated, mark handshaked = true.
            if msg.header().cmd == Cmd::GetCapabilities {
                handshaked = true;
            }
            if tx.write_all(msg.frame()).await.is_err() {
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

    use embassy_futures::join::join;
    use embedded_io_async::{ErrorKind, ErrorType, Read, Write};
    use rmk_types::action::KeyAction;
    use rmk_types::protocol::rynk::{LockStatus, MatrixState, ProtocolVersion};

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

    /// Bare 5-byte header; `payload_len = 0` is a complete empty request.
    fn header(cmd_raw: u16, seq: u8, payload_len: u16) -> Vec<u8> {
        let mut v = vec![0u8; RYNK_HEADER_SIZE];
        v[0..2].copy_from_slice(&cmd_raw.to_le_bytes());
        v[2] = seq;
        v[3..5].copy_from_slice(&payload_len.to_le_bytes());
        v
    }

    /// Split a captured response stream into `(cmd_raw, payload)` per frame.
    fn decode_frames(buf: &[u8]) -> Vec<(u16, &[u8])> {
        let mut out = Vec::new();
        let mut off = 0;
        while off + RYNK_HEADER_SIZE <= buf.len() {
            let cmd = u16::from_le_bytes([buf[off], buf[off + 1]]);
            let len = u16::from_le_bytes([buf[off + 3], buf[off + 4]]) as usize;
            let start = off + RYNK_HEADER_SIZE;
            out.push((cmd, &buf[start..start + len]));
            off = start + len;
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

        // Locked probe, status, unlock poll, unlocked probe.
        let mut stream = header(Cmd::GetMatrixState.raw(), 0, 0);
        stream.extend_from_slice(&header(Cmd::GetLockStatus.raw(), 1, 0));
        stream.extend_from_slice(&header(Cmd::UnlockPoll.raw(), 2, 0));
        stream.extend_from_slice(&header(Cmd::GetMatrixState.raw(), 3, 0));
        let mut chunks = VecDeque::new();
        chunks.push_back(stream);
        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };
        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(resp.len(), 4, "one reply per request");

        // Locked matrix reads reject instead of returning an empty bitmap.
        assert_eq!(resp[0].0, Cmd::GetMatrixState.raw());
        assert_eq!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(resp[0].1).unwrap(),
            Err(RynkError::Locked),
            "keystroke exfiltration is gated"
        );

        // Lock status is open while locked.
        let status: LockStatus = postcard::from_bytes::<Result<LockStatus, RynkError>>(resp[1].1)
            .unwrap()
            .unwrap();
        assert!(status.locked);
        assert_eq!(
            status.key_positions.as_slice(),
            &[(0, 0)],
            "challenge advertised while locked"
        );

        // Held challenge key unlocks.
        let polled: LockStatus = postcard::from_bytes::<Result<LockStatus, RynkError>>(resp[2].1)
            .unwrap()
            .unwrap();
        assert!(!polled.locked, "poll with challenge key held unlocks");
        assert_eq!(polled.remaining_keys, 0);

        // Gated command succeeds after unlock.
        assert!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(resp[3].1)
                .unwrap()
                .is_ok(),
            "gated command served once unlocked"
        );

        // New session starts locked again.
        let mut chunks2 = VecDeque::new();
        chunks2.push_back(header(Cmd::GetMatrixState.raw(), 0, 0));
        let mut rx2 = ChunkRead { chunks: chunks2 };
        let mut tx2 = VecWrite { captured: Vec::new() };
        block_on(service.run_session(&mut rx2, &mut tx2));

        let resp2 = decode_frames(&tx2.captured);
        assert_eq!(resp2.len(), 1);
        assert_eq!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(resp2[0].1).unwrap(),
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

        let mut stream_a = header(Cmd::UnlockPoll.raw(), 0x11, 0);
        stream_a.extend_from_slice(&header(Cmd::GetLockStatus.raw(), 0x12, 0));
        stream_a.extend_from_slice(&header(Cmd::Lock.raw(), 0x13, 0));
        stream_a.extend_from_slice(&header(Cmd::GetMatrixState.raw(), 0x14, 0));
        let mut chunks_a = VecDeque::new();
        chunks_a.push_back(stream_a);
        let mut rx_a = ChunkRead { chunks: chunks_a };
        let mut tx_a = VecWrite { captured: Vec::new() };

        let mut stream_b = header(Cmd::GetLockStatus.raw(), 0x21, 0);
        stream_b.extend_from_slice(&header(Cmd::UnlockPoll.raw(), 0x22, 0));
        stream_b.extend_from_slice(&header(Cmd::GetMatrixState.raw(), 0x23, 0));
        let mut chunks_b = VecDeque::new();
        chunks_b.push_back(stream_b);
        let mut rx_b = ChunkRead { chunks: chunks_b };
        let mut tx_b = VecWrite { captured: Vec::new() };

        block_on(join(
            service.run_session(&mut rx_a, &mut tx_a),
            service.run_session(&mut rx_b, &mut tx_b),
        ));

        let responses_a = decode_frames(&tx_a.captured);
        assert_eq!(responses_a.len(), 4);
        let unlocked_a = postcard::from_bytes::<Result<LockStatus, RynkError>>(responses_a[0].1)
            .unwrap()
            .unwrap();
        assert!(!unlocked_a.locked);
        let status_a = postcard::from_bytes::<Result<LockStatus, RynkError>>(responses_a[1].1)
            .unwrap()
            .unwrap();
        assert!(!status_a.locked);
        assert_eq!(
            postcard::from_bytes::<Result<(), RynkError>>(responses_a[2].1).unwrap(),
            Ok(())
        );
        assert_eq!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(responses_a[3].1).unwrap(),
            Err(RynkError::Locked),
        );

        let responses_b = decode_frames(&tx_b.captured);
        assert_eq!(responses_b.len(), 3);
        let locked_b = postcard::from_bytes::<Result<LockStatus, RynkError>>(responses_b[0].1)
            .unwrap()
            .unwrap();
        assert!(locked_b.locked, "session A does not unlock session B");
        let unlocked_b = postcard::from_bytes::<Result<LockStatus, RynkError>>(responses_b[1].1)
            .unwrap()
            .unwrap();
        assert!(!unlocked_b.locked);
        assert!(
            postcard::from_bytes::<Result<MatrixState, RynkError>>(responses_b[2].1)
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
        chunks.push_back(header(Cmd::GetMatrixState.raw(), 0, 0));
        let mut rx = ChunkRead { chunks };
        let mut tx = VecWrite { captured: Vec::new() };
        block_on(service.run_session(&mut rx, &mut tx));

        let resp = decode_frames(&tx.captured);
        assert_eq!(resp.len(), 1);
        let state: MatrixState = postcard::from_bytes::<Result<MatrixState, RynkError>>(resp[0].1)
            .unwrap()
            .unwrap();
        assert_eq!(&state.pressed_bitmap[..4], &[0x01, 0x02, 0x40, 0x20]);
        assert!(state.pressed_bitmap[4..].iter().all(|&b| b == 0));
    }

    /// Pipelined frames split across reads must both be dispatched.
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

        // Header plus `Ok(ProtocolVersion)`.
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

    /// Coalesced frames must drain before EOF.
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

    /// Topic-range requests are drained without creating phantom topic replies.
    #[test]
    fn run_session_drops_topic_range_request_without_reply() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // Topic-range request followed by a real request in one chunk.
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

    /// Oversized topic frames still draw no reply.
    #[test]
    fn run_session_oversized_topic_frame_draws_no_reply() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // No payload follows, so drain hits EOF.
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

    /// Oversized normal requests reply `Malformed` with cmd/seq preserved.
    #[test]
    fn run_session_oversized_request_replies_malformed() {
        let mut behavior = BehaviorConfig::default();
        let positional: PositionalConfig<1, 1> = PositionalConfig::default();
        let mut data: KeymapData<1, 1, 1, 0> = KeymapData::new([[[KeyAction::No]]]);
        let keymap = block_on(KeyMap::new(&mut data, &mut behavior, &positional));
        let config = RmkConfig::default();
        let service = RynkService::new(&keymap, &config);

        // No payload follows, so drain hits EOF.
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
