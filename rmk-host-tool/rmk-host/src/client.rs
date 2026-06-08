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
    PeripheralStatus, ProtocolVersion, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkMessage, SetComboRequest,
    SetEncoderRequest, SetForkRequest, SetKeyRequest, SetMacroRequest, SetMorseRequest, StorageResetMode,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::transport::{Transport, TransportError};

/// Queued topic frames before dropping the oldest.
const EVENT_QUEUE_CAPACITY: usize = 64;

/// A raw topic frame (server → host push), delivered via [`Client::next_event`].
#[derive(Debug, Clone)]
pub struct TopicFrame {
    pub cmd: Cmd,
    pub payload: Vec<u8>,
}

/// Errors from one request round trip.
#[derive(Debug, Error)]
pub enum RequestError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The firmware accepted the request but answered with an error.
    #[error("device rejected {0:?}")]
    Rejected(RynkError),
    #[error("request encode failed for {0:?} (request exceeds tx buffer?)")]
    Encode(Cmd),
    /// The encoded request frame is larger than the device's advertised
    /// [`max_payload_size`](DeviceCapabilities::max_payload_size)
    #[error("request {cmd:?} frame is {frame_len} bytes; device accepts at most {max}")]
    TooLarge { cmd: Cmd, frame_len: usize, max: usize },
    #[error("response decode failed for {cmd:?}: {source}")]
    Deserialize { cmd: Cmd, source: postcard::Error },
    #[error("response for {cmd:?} had trailing bytes")]
    TrailingBytes { cmd: Cmd },
    #[error("response cmd mismatch: sent {sent:?}, got {got:?}")]
    CmdMismatch { sent: Cmd, got: Cmd },
    /// A topic-range `Cmd` was passed to a request method — topics are
    /// server→host push only.
    #[error("{0:?} is a topic, not a request")]
    TopicCmd(Cmd),
    /// The cached device capabilities say this command's feature is absent, so
    /// the client rejected it locally without touching the wire — distinct from
    /// a firmware [`Rejected`](Self::Rejected) reply.
    #[error("device does not support {0:?}: {1}")]
    Unsupported(Cmd, &'static str),
}

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

/// A decoded firmware topic push (server → host), delivered by
/// [`Client::next_event`].
///
/// Mirrors the firmware's topic set. [`Event::Unknown`] carries a topic this
/// build doesn't recognize or one whose payload failed to decode, so a
/// forward-compatible frame surfaces instead of being silently dropped.
///
/// Topics are **best-effort**: the link can drop a push (a full in-client queue
/// — see [`Client::events_dropped`] — or, on BLE, an OS-level notification drop
/// the client cannot observe). When a current value matters, read it with the
/// matching `Get*` call ([`Client::get_connection_status`], [`Client::get_wpm`],
/// …), which the protocol provides for exactly this.
#[derive(Debug, Clone)]
pub enum Event {
    /// Active layer changed.
    LayerChange(u8),
    /// Words-per-minute estimate updated.
    WpmUpdate(u16),
    /// Connection status changed (same payload as [`Client::get_connection_status`]).
    ConnectionChange(ConnectionStatus),
    /// Sleep state changed.
    SleepState(bool),
    /// Host LED indicator (caps/num/scroll lock, …) changed.
    LedIndicator(LedIndicator),
    /// Battery status changed (BLE firmware only).
    BatteryStatus(BatteryStatus),
    /// A topic this build doesn't recognize, or one whose payload failed to decode.
    Unknown(TopicFrame),
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
    /// Set once the link is unrecoverable; every call then fails fast until the
    /// client is dropped and rebuilt.
    dead: bool,
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
    /// Largest frame either side may put on the wire — header + the device's
    /// advertised [`max_payload_size`](DeviceCapabilities::max_payload_size),
    /// set at connect time.
    max_frame_size: usize,
}

impl<T: Transport> Client<T> {
    /// Build an unhandshaked client.
    fn new(transport: T) -> Self {
        Self {
            transport,
            rx_buf: Vec::with_capacity(4096),
            next_seq: 1,
            dead: false,
            events: VecDeque::new(),
            events_dropped: 0,
            tx_buf: vec![0u8; RYNK_MIN_BUFFER_SIZE],
            // Construction-only placeholder; `connect` overwrites it with the
            // handshake value before handing the client out.
            protocol_version: ProtocolVersion::CURRENT,
            capabilities: None,
            max_frame_size: RYNK_MIN_BUFFER_SIZE,
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

        let caps: DeviceCapabilities = client.request_raw(Cmd::GetCapabilities, &()).await?;
        // The device's frames are at most header + max_payload_size
        client.max_frame_size = RYNK_HEADER_SIZE + caps.max_payload_size as usize;
        client.capabilities = Some(caps);
        Ok(client)
    }

    /// Cached capability snapshot from connect time.
    ///
    /// Stored as `Option` only to cover the brief unhandshaked window inside
    /// [`connect`](Self::connect): `new` is private and `connect` sets this before
    /// returning `Ok`, so once a caller holds a `Client` it is always `Some` and
    /// the `expect` cannot fire. (Building the client incrementally — rather than
    /// after the handshake — is what lets topic frames arriving *during* the
    /// handshake be queued instead of dropped.)
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
        !self.dead
    }

    /// Count of topics evicted from the in-client queue because it was full
    /// while no consumer was draining [`next_event`](Self::next_event).
    ///
    /// Counts **only** in-client queue overflow — not OS/BLE-level notification
    /// drops below the transport, which the client cannot observe. Treat topics
    /// as best-effort (see [`Event`]) and re-read current values with the
    /// matching `Get*` call when they matter.
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped
    }

    /// Clear the RX reassembly buffer after a caller-owned timeout or other
    /// external cancellation point. This does not reopen a dead link.
    pub fn resync(&mut self) {
        if !self.dead {
            self.rx_buf.clear();
        }
    }

    /// Read the next topic push, decoded into a typed [`Event`]. Queued topics
    /// are returned first. Cancel-safe.
    ///
    /// Topics are best-effort — see [`Event`] for recovering a missed push via
    /// the `Get*` snapshot calls.
    pub async fn next_event(&mut self) -> Result<Event, TransportError> {
        if let Some(ev) = self.events.pop_front() {
            return Ok(Self::decode_event(ev));
        }
        if self.dead {
            return Err(TransportError::Disconnected);
        }
        loop {
            match self.next_frame().await {
                Ok((cmd, _seq, payload)) if cmd.is_topic() => {
                    return Ok(Self::decode_event(TopicFrame { cmd, payload }));
                }
                // Stale response.
                Ok(_) => {}
                Err(e) => {
                    self.dead = true;
                    return Err(e);
                }
            }
        }
    }

    /// Reject a command locally when its required capability is absent, before
    /// touching the wire. Keeps capability gating in one place (the client holds
    /// the cached caps) instead of every caller re-deriving it.
    fn require_capability(&self, present: bool, cmd: Cmd, reason: &'static str) -> Result<(), RequestError> {
        if present {
            Ok(())
        } else {
            Err(RequestError::Unsupported(cmd, reason))
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
        if self.dead {
            return Err(TransportError::Disconnected.into());
        }
        let (seq, frame_len) = self.encode(cmd, req)?;
        if let Err(e) = self.transport.send(&self.tx_buf[..frame_len]).await {
            // A partial send desyncs the device; the link is unrecoverable.
            self.dead = true;
            return Err(e.into());
        }

        loop {
            let (got_cmd, got_seq, payload) = match self.next_frame().await {
                Ok(frame) => frame,
                Err(e) => {
                    self.dead = true;
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
        env.map_err(RequestError::Rejected)
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

    /// Decode a topic frame into a typed [`Event`], falling back to
    /// [`Event::Unknown`] for an unrecognized topic or a payload that fails to
    /// decode.
    fn decode_event(frame: TopicFrame) -> Event {
        let decoded = match frame.cmd {
            Cmd::LayerChange => Self::decode_topic::<u8>(&frame.payload).map(Event::LayerChange),
            Cmd::WpmUpdate => Self::decode_topic::<u16>(&frame.payload).map(Event::WpmUpdate),
            Cmd::ConnectionChange => {
                Self::decode_topic::<ConnectionStatus>(&frame.payload).map(Event::ConnectionChange)
            }
            Cmd::SleepState => Self::decode_topic::<bool>(&frame.payload).map(Event::SleepState),
            Cmd::LedIndicator => Self::decode_topic::<LedIndicator>(&frame.payload).map(Event::LedIndicator),
            Cmd::BatteryStatusTopic => Self::decode_topic::<BatteryStatus>(&frame.payload).map(Event::BatteryStatus),
            _ => None,
        };
        decoded.unwrap_or(Event::Unknown(frame))
    }

    /// Decode a topic payload, lenient about trailing bytes so a newer firmware
    /// that appends fields to a topic still decodes on an older host.
    fn decode_topic<V: DeserializeOwned>(payload: &[u8]) -> Option<V> {
        postcard::take_from_bytes::<V>(payload).ok().map(|(v, _)| v)
    }

    /// Send one request frame without waiting for a reply.
    async fn send_no_reply<Req: Serialize>(&mut self, cmd: Cmd, req: &Req) -> Result<(), RequestError> {
        if self.dead {
            return Err(TransportError::Disconnected.into());
        }
        let (_, frame_len) = self.encode(cmd, req)?;
        if let Err(e) = self.transport.send(&self.tx_buf[..frame_len]).await {
            self.dead = true;
            return Err(e.into());
        }
        Ok(())
    }

    /// Read the next complete frame.
    async fn next_frame(&mut self) -> Result<(Cmd, u8, Vec<u8>), TransportError> {
        loop {
            if self.rx_buf.len() >= RYNK_HEADER_SIZE {
                let cmd = Cmd::from_le_bytes([self.rx_buf[0], self.rx_buf[1]]);
                let seq = self.rx_buf[2];
                let payload_len = u16::from_le_bytes([self.rx_buf[3], self.rx_buf[4]]) as usize;
                let frame_len = RYNK_HEADER_SIZE + payload_len;

                // Unreachable under a conforming peer: the device never emits a
                // frame larger than the `max_frame_size` it advertised. Reaching
                // here means the byte stream is corrupt or desynced — clear and
                // re-sync from the next chunk.
                if frame_len > self.max_frame_size {
                    log::debug!("rynk: oversized frame header, dropping {} bytes", self.rx_buf.len());
                    self.rx_buf.clear();
                    continue;
                }

                if self.rx_buf.len() >= frame_len {
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

        if frame_len > self.max_frame_size {
            return Err(RequestError::TooLarge {
                cmd,
                frame_len,
                max: self.max_frame_size,
            });
        }
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

    /// Read battery status. BLE firmware only ([`DeviceCapabilities::ble_enabled`]);
    /// returns [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn get_battery_status(&mut self) -> Result<BatteryStatus, RequestError> {
        self.require_capability(
            self.capabilities().ble_enabled,
            Cmd::GetBatteryStatus,
            "BLE not enabled",
        )?;
        self.request_raw(Cmd::GetBatteryStatus, &()).await
    }

    /// Read one split peripheral's status by slot. Split BLE keyboards only
    /// ([`DeviceCapabilities::is_split`] and `ble_enabled`); returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn get_peripheral_status(&mut self, slot: u8) -> Result<PeripheralStatus, RequestError> {
        self.require_capability(
            self.capabilities().is_split && self.capabilities().ble_enabled,
            Cmd::GetPeripheralStatus,
            "not a split BLE keyboard",
        )?;
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

    /// Read BLE status (active profile, connection state). BLE firmware only
    /// ([`DeviceCapabilities::ble_enabled`]); returns [`RequestError::Unsupported`]
    /// otherwise, without touching the wire.
    pub async fn get_ble_status(&mut self) -> Result<BleStatus, RequestError> {
        self.require_capability(self.capabilities().ble_enabled, Cmd::GetBleStatus, "BLE not enabled")?;
        self.request_raw(Cmd::GetBleStatus, &()).await
    }

    /// Switch to a BLE profile by slot. BLE firmware only; returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn switch_ble_profile(&mut self, slot: u8) -> Result<(), RequestError> {
        self.require_capability(
            self.capabilities().ble_enabled,
            Cmd::SwitchBleProfile,
            "BLE not enabled",
        )?;
        self.request_raw(Cmd::SwitchBleProfile, &slot).await
    }

    /// Clear (unbond) a BLE profile by slot. Tears down the active link if it
    /// targets the connected profile. BLE firmware only; returns
    /// [`RequestError::Unsupported`] otherwise, without touching the wire.
    pub async fn clear_ble_profile(&mut self, slot: u8) -> Result<(), RequestError> {
        self.require_capability(self.capabilities().ble_enabled, Cmd::ClearBleProfile, "BLE not enabled")?;
        self.request_raw(Cmd::ClearBleProfile, &slot).await
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::VecDeque;
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

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
    async fn unknown_cmd_drained_by_len() {
        let mut chunk = header(0x7fff, 0xEE, 5);
        chunk.extend_from_slice(&[1, 2, 3, 4, 5]);
        chunk.extend_from_slice(&reply(Cmd::GetWpm, 1, 42u16));
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
    }

    #[tokio::test]
    async fn unknown_topic_cmd_queued_by_len() {
        let mut chunk = header(0x80ff, 0, 3);
        chunk.extend_from_slice(&[1, 2, 3]);
        chunk.extend_from_slice(&reply(Cmd::GetWpm, 1, 42u16));
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
        let ev = c.next_event().await.unwrap();
        assert!(matches!(ev, Event::Unknown(ref f) if f.cmd == Cmd::from_raw(0x80ff) && f.payload == [1, 2, 3]));
    }

    #[tokio::test]
    async fn unknown_response_cmd_mismatch_detected() {
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::from_raw(0x7fff), 1, 42u16))]);
        let r = c.get_wpm().await;
        assert!(matches!(
            r,
            Err(RequestError::CmdMismatch {
                sent: Cmd::GetWpm,
                got,
            }) if got == Cmd::from_raw(0x7fff)
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn caller_timeout_then_resyncs_phantom_frame() {
        let mut c = raw_client(vec![
            Step::Chunk(header(Cmd::GetWpm.raw(), 0xEE, 100)),
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
        assert!(matches!(ev, Event::LayerChange(3)));
    }

    #[tokio::test]
    async fn next_event_reads_from_link() {
        let mut c = raw_client(vec![Step::Chunk(topic(Cmd::LayerChange, 7u8))]);
        let ev = c.next_event().await.unwrap();
        assert!(matches!(ev, Event::LayerChange(7)));
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
            Step::Chunk(header(Cmd::LayerChange.raw(), 0, 1)), // topic header, payload pending
            Step::Hang,
            Step::Chunk(tail),
        ]);
        let cancelled = timeout(Duration::from_millis(10), c.next_event()).await;
        assert!(cancelled.is_err());
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
        let ev = c.next_event().await.unwrap();
        assert!(matches!(ev, Event::LayerChange(7)));
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
    async fn capability_gate_rejects_without_wire_send() {
        // caps() reports ble_enabled = false. A BLE-only call must reject locally
        // with `Unsupported` and consume NO transport step — the mock has none
        // left after the handshake, so a wire send would surface `Disconnected`.
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert!(!client.capabilities().ble_enabled);
        let r = client.get_battery_status().await;
        assert!(matches!(r, Err(RequestError::Unsupported(Cmd::GetBatteryStatus, _))));
        assert!(client.is_alive(), "a locally-gated reject must not kill the link");
    }

    #[tokio::test]
    async fn oversized_request_rejected_locally() {
        // Firmware advertises a tiny max_payload_size. A request whose encoded
        // frame exceeds it must be rejected locally with `TooLarge`, consuming
        // NO transport step (the mock has none left after the handshake, so a
        // wire send would surface `Disconnected`) and without killing the link.
        let mut tiny = caps();
        tiny.max_payload_size = 4;
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, tiny)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        let r = client.set_key(0, 0, 0, KeyAction::Morse(3)).await;
        assert!(matches!(
            r,
            Err(RequestError::TooLarge {
                cmd: Cmd::SetKeyAction,
                ..
            })
        ));
        assert!(
            client.is_alive(),
            "a locally-rejected oversized request must not kill the link"
        );
    }

    #[tokio::test]
    async fn next_event_decodes_typed_payload() {
        // A ConnectionChange topic must decode into the typed Event variant.
        let status = ConnectionStatus {
            preferred: ConnectionType::Ble,
            ..Default::default()
        };
        let mut c = raw_client(vec![Step::Chunk(topic(Cmd::ConnectionChange, status))]);
        let ev = c.next_event().await.unwrap();
        match ev {
            Event::ConnectionChange(s) => assert_eq!(s.preferred, ConnectionType::Ble),
            other => panic!("expected ConnectionChange, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn next_event_undecodable_payload_is_unknown() {
        // A known topic cmd whose payload can't decode (LayerChange needs 1 byte,
        // here it's empty) surfaces as Event::Unknown rather than being dropped.
        let mut c = raw_client(vec![Step::Chunk(header(Cmd::LayerChange.raw(), 0, 0))]);
        let ev = c.next_event().await.unwrap();
        assert!(matches!(ev, Event::Unknown(ref f) if f.cmd == Cmd::LayerChange && f.payload.is_empty()));
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
