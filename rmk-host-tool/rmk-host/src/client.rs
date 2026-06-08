//! Rynk client: handshake, typed requests, and topic delivery.
//!
//! Requests are serialized through `&mut self`. Topic frames seen while waiting
//! for replies are queued for [`Client::next_event`].

use std::collections::VecDeque;

use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::battery::BatteryStatus;
use rmk_types::ble::BleStatus;
use rmk_types::combo::Combo;
use rmk_types::connection::{ConnectionStatus, ConnectionType};
use rmk_types::fork::Fork;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::morse::Morse;
use rmk_types::protocol::rynk::{
    BehaviorConfig, Cmd, DeviceCapabilities, GetEncoderRequest, GetMacroRequest, KeyPosition, MacroData, MatrixState,
    PeripheralStatus, ProtocolVersion, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RYNK_TOPIC_BIT, RynkError, RynkMessage,
    SetComboRequest, SetEncoderRequest, SetForkRequest, SetKeyRequest, SetMacroRequest, SetMorseRequest,
    StorageResetMode,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::transport::{RequestError, TopicFrame, Transport, TransportError};

/// Queued topic frames before dropping the oldest.
const EVENT_QUEUE_CAPACITY: usize = 64;

/// Largest frame accepted from the device.
const MAX_FRAME_SIZE: usize = 4 * RYNK_MIN_BUFFER_SIZE;

/// Errors that can happen during [`Client::connect`].
#[derive(Debug, Error)]
pub enum ConnectError {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("handshake request failed: {0}")]
    Request(#[from] RequestError),
    /// Candidate ports were found, but none answered the handshake.
    #[error("{probed} candidate port(s) found, none answered the Rynk handshake (last: {last})")]
    NoResponsiveDevice { probed: usize, last: Box<ConnectError> },
    #[error(
        "protocol version mismatch — firmware is v{firmware_major}.{firmware_minor}, this tool supports up to \
         v{host_major}.{host_max_minor}. Update the tool, or flash firmware that matches it."
    )]
    VersionMismatch {
        firmware_major: u8,
        firmware_minor: u8,
        host_major: u8,
        host_max_minor: u8,
    },
}

/// Link lifecycle. `Dead` is terminal: rebuild the client.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LinkState {
    Ready,
    /// The link is closed; every call fails fast.
    Dead,
}

/// Rynk client over a [`Transport`].
///
/// Requests are cancel-safe once the send completes — cancelling a request
/// future mid-send can leave a partial frame and desync the device until
/// reconnect. [`next_event`](Self::next_event) is always cancel-safe.
pub struct Client<T: Transport> {
    transport: T,
    /// RX reassembly buffer.
    rx_buf: Vec<u8>,
    /// Request SEQ, cycling through `1..=255`.
    next_seq: u8,
    link: LinkState,
    /// Queued topic frames.
    events: VecDeque<TopicFrame>,
    /// Topics dropped from a full queue.
    events_dropped: u64,
    /// Reusable TX scratch.
    tx_buf: Vec<u8>,
    /// Firmware protocol version, from the handshake.
    protocol_version: ProtocolVersion,
    /// Set after handshake.
    capabilities: Option<DeviceCapabilities>,
}

impl<T: Transport> Client<T> {
    /// Build an unhandshaked client.
    fn new(transport: T) -> Self {
        Self {
            transport,
            rx_buf: Vec::with_capacity(4096),
            next_seq: 1,
            link: LinkState::Ready,
            events: VecDeque::new(),
            events_dropped: 0,
            tx_buf: vec![0u8; RYNK_MIN_BUFFER_SIZE],
            protocol_version: ProtocolVersion::CURRENT,
            capabilities: None,
        }
    }

    /// Handshake and read device capabilities.
    pub async fn connect(transport: T) -> Result<Self, ConnectError> {
        let mut client = Self::new(transport);
        let version: ProtocolVersion = client.request_raw(Cmd::GetVersion, &()).await?;

        let supported = ProtocolVersion::CURRENT;
        if version.major != supported.major || version.minor > supported.minor {
            return Err(ConnectError::VersionMismatch {
                firmware_major: version.major,
                firmware_minor: version.minor,
                host_major: supported.major,
                host_max_minor: supported.minor,
            });
        }
        client.protocol_version = version;

        let caps = client.request_raw(Cmd::GetCapabilities, &()).await?;
        client.capabilities = Some(caps);
        Ok(client)
    }

    /// Cached capability snapshot from connect time.
    pub fn capabilities(&self) -> &DeviceCapabilities {
        self.capabilities
            .as_ref()
            .expect("capabilities are set before connect hands the client out")
    }

    /// Firmware protocol version reported at connect time.
    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    /// The owned transport, e.g. to read connection identity.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// `false` once the link is dead — drop the client and reconnect.
    pub fn is_alive(&self) -> bool {
        self.link != LinkState::Dead
    }

    /// Topics dropped because the event queue was full while no consumer
    /// was draining [`next_event`](Self::next_event).
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped
    }

    /// Clear the RX reassembly buffer after a caller-owned timeout or other
    /// external cancellation point. This does not reopen a dead link.
    pub fn resync(&mut self) {
        if self.link != LinkState::Dead {
            self.rx_buf.clear();
        }
    }

    /// Read the next topic frame. Queued topics are returned first. Cancel-safe.
    pub async fn next_event(&mut self) -> Result<TopicFrame, TransportError> {
        if let Some(ev) = self.events.pop_front() {
            return Ok(ev);
        }
        if self.link == LinkState::Dead {
            return Err(TransportError::Disconnected);
        }
        loop {
            match self.next_frame().await {
                Ok((cmd, _seq, payload)) if cmd.is_topic() => return Ok(TopicFrame { cmd, payload }),
                // Stale response.
                Ok(_) => {}
                Err(e) => {
                    self.link = LinkState::Dead;
                    return Err(e);
                }
            }
        }
    }

    /// One request/response round-trip with the response envelope unwrapped:
    /// `Ok(v)` on accept, `Err(RequestError::Rejected)` on device rejection.
    /// The typed methods are thin wrappers over this; call it directly for a
    /// `Cmd` without a typed method yet (e.g. bulk).
    pub async fn request_raw<Req: Serialize, Resp: DeserializeOwned>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> Result<Resp, RequestError> {
        if cmd.is_topic() {
            return Err(RequestError::TopicCmd(cmd));
        }
        if self.link == LinkState::Dead {
            return Err(TransportError::Disconnected.into());
        }
        let (seq, frame_len) = self.encode(cmd, req)?;
        if let Err(e) = self.transport.send(&self.tx_buf[..frame_len]).await {
            // A partial send desyncs the device; the link is unrecoverable.
            self.link = LinkState::Dead;
            return Err(e.into());
        }

        loop {
            let (got_cmd, got_seq, payload) = match self.next_frame().await {
                Ok(frame) => frame,
                Err(e) => {
                    self.link = LinkState::Dead;
                    return Err(RequestError::Transport(e));
                }
            };
            if got_cmd.is_topic() {
                self.queue_topic(got_cmd, payload);
            } else if got_seq == seq {
                if got_cmd != cmd {
                    return Err(RequestError::CmdMismatch {
                        sent: cmd,
                        got: got_cmd,
                    });
                }
                return Self::decode_response(cmd, &payload);
            }
            // Stale response.
        }
    }

    /// Decode the `Result<Resp, RynkError>` response envelope, flattening a
    /// device rejection into [`RequestError::Rejected`]. Trailing bytes mean a
    /// response longer than `Resp` — a wire/type mismatch, so reject them.
    fn decode_response<Resp: DeserializeOwned>(cmd: Cmd, payload: &[u8]) -> Result<Resp, RequestError> {
        let (env, rest) = postcard::take_from_bytes::<Result<Resp, RynkError>>(payload)
            .map_err(|source| RequestError::Deserialize { cmd, source })?;
        if !rest.is_empty() {
            return Err(RequestError::TrailingBytes { cmd });
        }
        Ok(env?)
    }

    fn queue_topic(&mut self, cmd: Cmd, payload: Vec<u8>) {
        if self.events.len() == EVENT_QUEUE_CAPACITY {
            self.events.pop_front();
            self.events_dropped += 1;
            log::debug!(
                "rynk: event queue full, dropped oldest topic ({} total)",
                self.events_dropped
            );
        }
        self.events.push_back(TopicFrame { cmd, payload });
    }

    /// Send one request frame without waiting for a reply.
    async fn send_no_reply<Req: Serialize>(&mut self, cmd: Cmd, req: &Req) -> Result<(), RequestError> {
        if self.link == LinkState::Dead {
            return Err(TransportError::Disconnected.into());
        }
        let (_, frame_len) = self.encode(cmd, req)?;
        if let Err(e) = self.transport.send(&self.tx_buf[..frame_len]).await {
            self.link = LinkState::Dead;
            return Err(e.into());
        }
        Ok(())
    }

    /// Read the next complete frame.
    async fn next_frame(&mut self) -> Result<(Cmd, u8, Vec<u8>), TransportError> {
        loop {
            if self.rx_buf.len() >= RYNK_HEADER_SIZE {
                let cmd_raw = u16::from_le_bytes([self.rx_buf[0], self.rx_buf[1]]);
                let cmd = Cmd::from_repr(cmd_raw);
                let seq = self.rx_buf[2];
                let payload_len = u16::from_le_bytes([self.rx_buf[3], self.rx_buf[4]]) as usize;
                let frame_len = RYNK_HEADER_SIZE + payload_len;

                if (cmd.is_none() && cmd_raw & RYNK_TOPIC_BIT == 0) || frame_len > MAX_FRAME_SIZE {
                    log::debug!(
                        "rynk: malformed header (cmd={cmd_raw:#06x}), dropping {} bytes",
                        self.rx_buf.len()
                    );
                    self.rx_buf.clear();
                    continue;
                }

                if self.rx_buf.len() >= frame_len {
                    let Some(cmd) = cmd else {
                        // Unknown topic from a newer firmware.
                        self.rx_buf.drain(..frame_len);
                        continue;
                    };
                    let payload = self.rx_buf[RYNK_HEADER_SIZE..frame_len].to_vec();
                    self.rx_buf.drain(..frame_len);
                    return Ok((cmd, seq, payload));
                }
            }

            let chunk = self.transport.recv().await?;
            self.rx_buf.extend_from_slice(&chunk);
        }
    }

    /// Encode one frame into the TX scratch.
    fn encode<Req: Serialize>(&mut self, cmd: Cmd, req: &Req) -> Result<(u8, usize), RequestError> {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        if self.next_seq == 0 {
            self.next_seq = 1;
        }
        let frame_len = RynkMessage::build(&mut self.tx_buf, cmd, seq, req)
            .map_err(|_| RequestError::Encode(cmd))?
            .frame_len();
        Ok((seq, frame_len))
    }
}

/// Typed operation methods. Each returns the response value directly; a device
/// rejection surfaces as [`RequestError::Rejected`], so `?` propagates both
/// transport and firmware failures.
impl<T: Transport> Client<T> {
    // ── system ──

    /// Read the firmware's protocol version.
    pub async fn get_version(&mut self) -> Result<ProtocolVersion, RequestError> {
        self.request_raw(Cmd::GetVersion, &()).await
    }

    /// Re-read the firmware's capability set. Prefer the cached
    /// [`Client::capabilities`] for the snapshot taken at connect time.
    pub async fn get_capabilities(&mut self) -> Result<DeviceCapabilities, RequestError> {
        self.request_raw(Cmd::GetCapabilities, &()).await
    }

    /// Reboot the device — fire-and-forget: the firmware resets before its
    /// session loop can reply, so `Ok(())` only means the request frame was
    /// handed to the link.
    pub async fn reboot(&mut self) -> Result<(), RequestError> {
        self.send_no_reply(Cmd::Reboot, &()).await
    }

    /// Jump to the bootloader (DFU mode) — fire-and-forget, same contract as
    /// [`reboot`](Self::reboot).
    pub async fn bootloader_jump(&mut self) -> Result<(), RequestError> {
        self.send_no_reply(Cmd::BootloaderJump, &()).await
    }

    /// Reset persistent storage. Current firmware implements only
    /// [`StorageResetMode::Full`] — a full wipe including saved keymap edits
    /// **and BLE bonds** — and rejects [`StorageResetMode::LayoutOnly`] with
    /// `Unimplemented`.
    pub async fn storage_reset(&mut self, mode: StorageResetMode) -> Result<(), RequestError> {
        self.request_raw(Cmd::StorageReset, &mode).await
    }

    // ── keymap ──

    /// Read one key's action.
    pub async fn get_key(&mut self, layer: u8, row: u8, col: u8) -> Result<KeyAction, RequestError> {
        self.request_raw(Cmd::GetKeyAction, &KeyPosition { layer, row, col })
            .await
    }

    /// Write one key's action and persist it to flash.
    pub async fn set_key(&mut self, layer: u8, row: u8, col: u8, action: KeyAction) -> Result<(), RequestError> {
        let req = SetKeyRequest {
            position: KeyPosition { layer, row, col },
            action,
        };
        self.request_raw(Cmd::SetKeyAction, &req).await
    }

    /// Read the currently selected default layer index.
    pub async fn get_default_layer(&mut self) -> Result<u8, RequestError> {
        self.request_raw(Cmd::GetDefaultLayer, &()).await
    }

    /// Set the default layer.
    pub async fn set_default_layer(&mut self, layer: u8) -> Result<(), RequestError> {
        self.request_raw(Cmd::SetDefaultLayer, &layer).await
    }

    /// Read both rotation actions for one encoder on one layer.
    pub async fn get_encoder(&mut self, encoder_id: u8, layer: u8) -> Result<EncoderAction, RequestError> {
        self.request_raw(Cmd::GetEncoderAction, &GetEncoderRequest { encoder_id, layer })
            .await
    }

    /// Set both rotation actions for one encoder on one layer.
    pub async fn set_encoder(&mut self, encoder_id: u8, layer: u8, action: EncoderAction) -> Result<(), RequestError> {
        let req = SetEncoderRequest {
            encoder_id,
            layer,
            action,
        };
        self.request_raw(Cmd::SetEncoderAction, &req).await
    }

    // ── combos / forks / morse / macros ──

    /// Read one combo entry by index.
    pub async fn get_combo(&mut self, index: u8) -> Result<Combo, RequestError> {
        self.request_raw(Cmd::GetCombo, &index).await
    }

    /// Write one combo entry by index.
    pub async fn set_combo(&mut self, index: u8, config: Combo) -> Result<(), RequestError> {
        self.request_raw(Cmd::SetCombo, &SetComboRequest { index, config })
            .await
    }

    /// Read one fork entry by index.
    pub async fn get_fork(&mut self, index: u8) -> Result<Fork, RequestError> {
        self.request_raw(Cmd::GetFork, &index).await
    }

    /// Write one fork entry by index.
    pub async fn set_fork(&mut self, index: u8, config: Fork) -> Result<(), RequestError> {
        self.request_raw(Cmd::SetFork, &SetForkRequest { index, config }).await
    }

    /// Read one morse entry by index.
    pub async fn get_morse(&mut self, index: u8) -> Result<Morse, RequestError> {
        self.request_raw(Cmd::GetMorse, &index).await
    }

    /// Write one morse entry by index.
    pub async fn set_morse(&mut self, index: u8, config: Morse) -> Result<(), RequestError> {
        self.request_raw(Cmd::SetMorse, &SetMorseRequest { index, config })
            .await
    }

    /// Read one chunk of macro data starting at byte `offset`. The firmware
    /// always replies with exactly its build-time chunk size, zero-filling
    /// past the end of its macro space — a short chunk is **not** an
    /// end-of-data signal; parse the macro encoding itself for termination.
    pub async fn get_macro(&mut self, index: u8, offset: u16) -> Result<MacroData, RequestError> {
        self.request_raw(Cmd::GetMacro, &GetMacroRequest { index, offset })
            .await
    }

    /// Write one chunk of macro data starting at byte `offset`. Writes past
    /// the end of the device's macro space are truncated by the firmware.
    pub async fn set_macro(&mut self, index: u8, offset: u16, data: MacroData) -> Result<(), RequestError> {
        self.request_raw(Cmd::SetMacro, &SetMacroRequest { index, offset, data })
            .await
    }

    // ── behavior ──

    /// Read the global behavior config.
    pub async fn get_behavior(&mut self) -> Result<BehaviorConfig, RequestError> {
        self.request_raw(Cmd::GetBehaviorConfig, &()).await
    }

    /// Write the global behavior config.
    pub async fn set_behavior(&mut self, config: BehaviorConfig) -> Result<(), RequestError> {
        self.request_raw(Cmd::SetBehaviorConfig, &config).await
    }

    // ── status ──

    /// Read the currently active layer.
    pub async fn get_current_layer(&mut self) -> Result<u8, RequestError> {
        self.request_raw(Cmd::GetCurrentLayer, &()).await
    }

    /// Read the matrix scan bitmap.
    pub async fn get_matrix_state(&mut self) -> Result<MatrixState, RequestError> {
        self.request_raw(Cmd::GetMatrixState, &()).await
    }

    /// Read battery status. Only meaningful on BLE firmware
    /// ([`DeviceCapabilities::ble_enabled`]).
    pub async fn get_battery_status(&mut self) -> Result<BatteryStatus, RequestError> {
        self.request_raw(Cmd::GetBatteryStatus, &()).await
    }

    /// Read one split peripheral's status by slot. Only meaningful on a split
    /// BLE keyboard ([`DeviceCapabilities::is_split`] and `ble_enabled`).
    pub async fn get_peripheral_status(&mut self, slot: u8) -> Result<PeripheralStatus, RequestError> {
        self.request_raw(Cmd::GetPeripheralStatus, &slot).await
    }

    /// Read the current words-per-minute estimate.
    pub async fn get_wpm(&mut self) -> Result<u16, RequestError> {
        self.request_raw(Cmd::GetWpm, &()).await
    }

    /// Read the firmware's sleep state.
    pub async fn get_sleep_state(&mut self) -> Result<bool, RequestError> {
        self.request_raw(Cmd::GetSleepState, &()).await
    }

    /// Read the host LED indicator state (caps/num/scroll lock, etc.).
    pub async fn get_led_indicator(&mut self) -> Result<LedIndicator, RequestError> {
        self.request_raw(Cmd::GetLedIndicator, &()).await
    }

    // ── connection ──

    /// Read the active connection type (USB / BLE).
    pub async fn get_connection_type(&mut self) -> Result<ConnectionType, RequestError> {
        self.request_raw(Cmd::GetConnectionType, &()).await
    }

    /// Read the full connection status — the same payload the `ConnectionChange`
    /// topic pushes, for recovering a missed push.
    pub async fn get_connection_status(&mut self) -> Result<ConnectionStatus, RequestError> {
        self.request_raw(Cmd::GetConnectionStatus, &()).await
    }

    /// Read BLE status (active profile, connection state). Only meaningful on
    /// BLE firmware ([`DeviceCapabilities::ble_enabled`]).
    pub async fn get_ble_status(&mut self) -> Result<BleStatus, RequestError> {
        self.request_raw(Cmd::GetBleStatus, &()).await
    }

    /// Switch to a BLE profile by slot.
    pub async fn switch_ble_profile(&mut self, slot: u8) -> Result<(), RequestError> {
        self.request_raw(Cmd::SwitchBleProfile, &slot).await
    }

    /// Clear (unbond) a BLE profile by slot. Tears down the active link if it
    /// targets the connected profile.
    pub async fn clear_ble_profile(&mut self, slot: u8) -> Result<(), RequestError> {
        self.request_raw(Cmd::ClearBleProfile, &slot).await
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::VecDeque;
    use std::time::Duration;

    use super::*;
    use tokio::time::timeout;

    enum Step {
        Chunk(Vec<u8>),
        Hang,
    }

    struct MockTransport {
        steps: VecDeque<Step>,
        fail_send: bool,
    }
    impl MockTransport {
        fn new(steps: Vec<Step>) -> Self {
            Self {
                steps: steps.into(),
                fail_send: false,
            }
        }
    }
    impl Transport for MockTransport {
        async fn send(&mut self, _frame: &[u8]) -> Result<(), TransportError> {
            if self.fail_send {
                return Err(TransportError::Io("send failed".into()));
            }
            Ok(())
        }
        async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
            match self.steps.pop_front() {
                Some(Step::Chunk(c)) => Ok(c),
                Some(Step::Hang) => std::future::pending().await,
                None => Err(TransportError::Disconnected),
            }
        }
    }

    fn raw_client(steps: Vec<Step>) -> Client<MockTransport> {
        Client::new(MockTransport::new(steps))
    }

    /// A bare frame: header + postcard(value). Used for raw headers and topics.
    fn frame<T: Serialize>(cmd: Cmd, seq: u8, value: &T) -> Vec<u8> {
        let mut buf = vec![0u8; RYNK_MIN_BUFFER_SIZE];
        let len = RynkMessage::build(&mut buf, cmd, seq, value).unwrap().frame_len();
        buf.truncate(len);
        buf
    }

    /// An `Ok` response frame, enveloped as the firmware sends it.
    fn reply<T: Serialize>(cmd: Cmd, seq: u8, value: T) -> Vec<u8> {
        frame(cmd, seq, &Ok::<T, RynkError>(value))
    }

    /// A topic push frame (bare payload, SEQ 0).
    fn topic<T: Serialize>(cmd: Cmd, value: T) -> Vec<u8> {
        frame(cmd, 0, &value)
    }

    fn header(cmd_raw: u16, seq: u8, len: u16) -> Vec<u8> {
        let c = cmd_raw.to_le_bytes();
        let l = len.to_le_bytes();
        vec![c[0], c[1], seq, l[0], l[1]]
    }

    fn caps() -> DeviceCapabilities {
        DeviceCapabilities {
            num_layers: 4,
            num_rows: 6,
            num_cols: 14,
            num_encoders: 0,
            max_combos: 8,
            max_combo_keys: 4,
            max_macros: 8,
            macro_space_size: 1024,
            max_morse: 4,
            max_patterns_per_key: 4,
            max_forks: 4,
            storage_enabled: true,
            lighting_enabled: false,
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: false,
            num_ble_profiles: 0,
            max_payload_size: 256,
            max_bulk_keys: 0,
            macro_chunk_size: 64,
            bulk_transfer_supported: false,
        }
    }

    // ── driver core ──

    #[tokio::test]
    async fn reply_round_trip() {
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::GetWpm, 1, 42u16))]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
    }

    #[tokio::test]
    async fn rejected_response_flattens() {
        let mut c = raw_client(vec![Step::Chunk(frame(
            Cmd::SetDefaultLayer,
            1,
            &Err::<(), RynkError>(RynkError::Invalid),
        ))]);
        let r = c.set_default_layer(9).await;
        assert!(matches!(r, Err(RequestError::Rejected(RynkError::Invalid))));
    }

    #[tokio::test]
    async fn trailing_bytes_rejected() {
        // A u16 reply with extra bytes — response longer than the type.
        let mut chunk = reply(Cmd::GetWpm, 1, 42u16);
        chunk[3] += 2; // bump the declared LEN
        chunk.extend_from_slice(&[0xAA, 0xBB]);
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let r = c.get_wpm().await;
        assert!(matches!(r, Err(RequestError::TrailingBytes { cmd: Cmd::GetWpm })));
    }

    #[tokio::test]
    async fn topic_cmd_to_request_rejected() {
        let mut c = raw_client(vec![]);
        let r = c.request_raw::<(), u8>(Cmd::LayerChange, &()).await;
        assert!(matches!(r, Err(RequestError::TopicCmd(Cmd::LayerChange))));
    }

    #[tokio::test]
    async fn malformed_header_clears_buffer() {
        let mut c = raw_client(vec![
            Step::Chunk(header(0x7fff, 0xEE, 5)),
            Step::Chunk(reply(Cmd::GetWpm, 1, 42u16)),
        ]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
    }

    #[tokio::test]
    async fn unknown_topic_skipped_by_len() {
        let mut chunk = header(0x80ff, 0, 3);
        chunk.extend_from_slice(&[1, 2, 3]);
        chunk.extend_from_slice(&reply(Cmd::GetWpm, 1, 42u16));
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
    }

    #[tokio::test(start_paused = true)]
    async fn caller_timeout_then_resyncs_phantom_frame() {
        let mut c = raw_client(vec![
            Step::Chunk(header(Cmd::GetWpm as u16, 0xEE, 100)),
            Step::Hang,
            Step::Chunk(reply(Cmd::GetWpm, 2, 42u16)),
        ]);
        let r1 = timeout(Duration::from_millis(10), c.get_wpm()).await;
        assert!(r1.is_err());
        c.resync();
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
    }

    #[tokio::test]
    async fn link_death_fails_fast() {
        let mut c = raw_client(vec![]);
        let r1 = c.get_wpm().await;
        assert!(matches!(r1, Err(RequestError::Transport(TransportError::Disconnected))));
        assert!(!c.is_alive());
        let r2 = c.get_wpm().await;
        assert!(matches!(r2, Err(RequestError::Transport(TransportError::Disconnected))));
        let ev = c.next_event().await;
        assert!(matches!(ev, Err(TransportError::Disconnected)));
    }

    #[tokio::test]
    async fn send_failure_marks_link_dead() {
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::GetWpm, 1, 42u16))]);
        c.transport.fail_send = true;
        let r = c.get_wpm().await;
        assert!(matches!(r, Err(RequestError::Transport(TransportError::Io(_)))));
        assert!(!c.is_alive(), "a failed send is unrecoverable");
        // Even with a readable reply queued, the dead link fails fast.
        let r2 = c.get_wpm().await;
        assert!(matches!(r2, Err(RequestError::Transport(TransportError::Disconnected))));
    }

    #[tokio::test]
    async fn topic_during_request_is_queued() {
        let mut chunk = topic(Cmd::LayerChange, 3u8);
        chunk.extend_from_slice(&reply(Cmd::GetWpm, 1, 42u16));
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
        let ev = c.next_event().await.unwrap();
        assert_eq!(ev.cmd, Cmd::LayerChange);
        assert_eq!(ev.payload, vec![3]);
    }

    #[tokio::test]
    async fn next_event_reads_from_link() {
        let mut c = raw_client(vec![Step::Chunk(topic(Cmd::LayerChange, 7u8))]);
        let ev = c.next_event().await.unwrap();
        assert_eq!(ev.cmd, Cmd::LayerChange);
        assert_eq!(ev.payload, vec![7]);
    }

    #[tokio::test]
    async fn stale_seq_reply_dropped() {
        let mut chunk = reply(Cmd::GetWpm, 0xEE, 99u16);
        chunk.extend_from_slice(&reply(Cmd::GetWpm, 1, 42u16));
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
    }

    #[tokio::test]
    async fn cmd_mismatch_detected() {
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::GetSleepState, 1, true))]);
        let r = c.get_wpm().await;
        assert!(matches!(
            r,
            Err(RequestError::CmdMismatch {
                sent: Cmd::GetWpm,
                got: Cmd::GetSleepState,
            })
        ));
    }

    // ── caller cancellation (cancel-safety) ──

    #[tokio::test(start_paused = true)]
    async fn caller_cancel_mid_reply_wait_then_next_request_ok() {
        // Request 1 parks waiting for a reply; the caller cancels it (external
        // timeout). Its late reply must be dropped and request 2 must succeed.
        let mut c = raw_client(vec![
            Step::Hang,
            Step::Chunk(reply(Cmd::GetWpm, 1, 11u16)), // late reply to request 1
            Step::Chunk(reply(Cmd::GetWpm, 2, 42u16)), // reply to request 2
        ]);
        let cancelled = timeout(Duration::from_millis(10), c.get_wpm()).await;
        assert!(cancelled.is_err(), "outer timeout cancels request 1 mid-wait");
        assert!(c.is_alive());
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
    }

    #[tokio::test(start_paused = true)]
    async fn caller_cancel_next_event_mid_reassembly_then_request_ok() {
        // A partial topic frame sits in the buffer when next_event is cancelled.
        // The next request finishes that topic (queued) and reads its own reply.
        let mut tail = vec![7u8]; // the LayerChange payload, arriving after cancel
        tail.extend_from_slice(&reply(Cmd::GetWpm, 1, 42u16));
        let mut c = raw_client(vec![
            Step::Chunk(header(Cmd::LayerChange as u16, 0, 1)), // topic header, payload pending
            Step::Hang,
            Step::Chunk(tail),
        ]);
        let cancelled = timeout(Duration::from_millis(10), c.next_event()).await;
        assert!(cancelled.is_err());
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
        let ev = c.next_event().await.unwrap();
        assert_eq!(ev.cmd, Cmd::LayerChange);
        assert_eq!(ev.payload, vec![7]);
    }

    // ── handshake ──

    #[tokio::test]
    async fn connect_handshake_loopback() {
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
            Step::Chunk(reply(Cmd::GetWpm, 3, 37u16)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert_eq!(client.capabilities().num_cols, 14);
        assert_eq!(client.protocol_version(), ProtocolVersion::CURRENT);
        assert_eq!(client.get_wpm().await.unwrap(), 37);
    }

    #[tokio::test]
    async fn connect_rejects_newer_major() {
        let newer = ProtocolVersion {
            major: ProtocolVersion::CURRENT.major + 1,
            minor: 0,
        };
        let t = MockTransport::new(vec![Step::Chunk(reply(Cmd::GetVersion, 1, newer))]);
        let err = Client::connect(t).await.err().expect("connect must fail");
        assert!(matches!(err, ConnectError::VersionMismatch { .. }));
    }

    #[tokio::test(start_paused = true)]
    async fn caller_can_timeout_silent_connect() {
        let t = MockTransport::new(vec![Step::Hang]);
        let err = timeout(Duration::from_millis(10), Client::connect(t)).await;
        assert!(err.is_err());
    }
}
