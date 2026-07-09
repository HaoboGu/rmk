//! Protocol driver for Rynk: framing, SEQ correlation, topic queueing, link lifecycle.
//!
//! [`Client`] drives the wire protocol over any byte link. It is
//! **version-independent**: it knows how to *talk* — frame, correlate, demux —
//! not *what to say*. The typed, version-specific surface (endpoint methods,
//! [`IncomingTopic`](crate::IncomingTopic) decoding) lives in `api.rs` as a
//! second impl block on [`Client`].
//!
//! ## Frame routing
//!
//! A frame is a 5-byte header (`CMD u16 | SEQ u8 | LEN u16`) + a postcard
//! payload — see the `rmk_types` Rynk ICD for the authoritative layout. The CMD
//! high bit marks a topic push, SEQ ties a response to the request that earned
//! it, and LEN delimits the frame. One inbound byte stream fans out three ways:
//!
//! ```text
//! SEND   request(&req) → encode + assign SEQ → write_all → transport
//!
//! RECV   transport → read (arbitrary chunks) → reassemble whole frames,
//!        then route each frame by its 5-byte header:
//!
//!          topic   CMD high bit set          → event queue, drained by next_event()
//!          reply   SEQ matches our request   → returned by request()
//!          stale   SEQ from a past request   → dropped
//! ```
//!
//! ## Link lifecycle
//!
//! - **Reassembly is cancel-safe** — a read lands in scratch and joins the RX
//!   buffer only once it completes, so dropping a read future (a caller timeout,
//!   then [`Client::resync`]) loses no delivered bytes.
//! - **A broken link latches dead** — EOF, an I/O error, or a failed mid-send
//!   (which leaves a partial frame and desyncs the peer) marks the link dead;
//!   every later call then fails fast instead of reading a corrupt stream.

use std::collections::VecDeque;

use embedded_io_async::{Read, Write};
use rmk_types::protocol::rynk::endpoint::Endpoint;
use rmk_types::protocol::rynk::{
    Cmd, DeviceCapabilities, ProtocolVersion, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkHeader,
    RynkMessage,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

/// Topic frames buffered before the oldest is dropped.
const EVENT_QUEUE_CAPACITY: usize = 64;

/// RX scratch filled per `read`; larger frames accumulate across reads.
const READ_SCRATCH_SIZE: usize = 4096;

/// A raw topic frame (server → host push), delivered by
/// [`Client::next_event`](crate::Client::next_event).
#[derive(Debug, Clone)]
pub struct TopicFrame {
    pub cmd: Cmd,
    pub payload: Vec<u8>,
}

/// Errors from Rynk host.
#[derive(Debug, Error)]
pub enum RynkHostError {
    #[error("transport disconnected")]
    Disconnected,
    #[error("io error: {0}")]
    Io(String),
    #[error("device not found: {0}")]
    DeviceNotFound(String),

    /// Protocol version mismatch.
    #[error(
        "protocol major version mismatch — firmware speaks v{firmware_major}.{firmware_minor}, this tool speaks \
         v{host_major}.x (currently v{host_major}.{host_max_minor}). Use a tool matching major {firmware_major}, or \
         flash firmware that matches this one."
    )]
    VersionMismatch {
        firmware_major: u8,
        firmware_minor: u8,
        host_major: u8,
        host_max_minor: u8,
    },

    // ── request round trip ──
    /// Firmware accepted the request but answered with an error.
    #[error("device rejected {0:?}")]
    Rejected(RynkError),
    #[error("request encode failed for {0:?} (request exceeds tx buffer?)")]
    Encode(Cmd),
    /// Encoded frame exceeds the device's advertised
    /// [`max_payload_size`](DeviceCapabilities::max_payload_size).
    #[error("request {cmd:?} frame is {frame_len} bytes; device accepts at most {max}")]
    TooLarge { cmd: Cmd, frame_len: usize, max: usize },
    #[error("response decode failed for {cmd:?}: {source}")]
    Deserialize { cmd: Cmd, source: postcard::Error },
    /// The assembled `GetLayout` blob failed to inflate or postcard-decode into
    /// `LayoutInfo` — distinct from a single frame's [`Deserialize`](Self::Deserialize).
    #[error("layout blob decode failed: {0}")]
    Layout(String),
    #[error("response for {cmd:?} had trailing bytes")]
    TrailingBytes { cmd: Cmd },
    #[error("response cmd mismatch: sent {sent:?}, got {got:?}")]
    CmdMismatch { sent: Cmd, got: Cmd },
    /// A topic-range `Cmd` was passed to a request method; topics are
    /// server→host push only.
    #[error("{0:?} is a topic, not a request")]
    TopicCmd(Cmd),
    /// Cached capabilities say this command's feature is absent, so the client
    /// rejected it locally without touching the wire — unlike a firmware
    /// [`Rejected`](Self::Rejected) reply.
    #[error("device does not support {0:?}: {1}")]
    Unsupported(Cmd, &'static str),
}

/// Bridge host errors into a JS `Error` whose `name` is a stable kind string the
/// web client switches on, so wasm methods can surface `RynkHostError` with `?`.
#[cfg(feature = "wasm")]
impl From<RynkHostError> for wasm_bindgen::JsValue {
    fn from(e: RynkHostError) -> Self {
        let kind = match &e {
            RynkHostError::Disconnected => "Disconnected",
            RynkHostError::Io(_) => "TransportError",
            RynkHostError::DeviceNotFound(_) => "DeviceNotFound",
            RynkHostError::Rejected(_) => "Rejected",
            RynkHostError::Unsupported(..) => "Unsupported",
            RynkHostError::VersionMismatch { .. } => "VersionMismatch",
            RynkHostError::Encode(_) => "RequestEncodeError",
            RynkHostError::TooLarge { .. } => "RequestTooLarge",
            RynkHostError::Deserialize { .. } => "ResponseDecodeError",
            RynkHostError::Layout(_) => "LayoutDecodeError",
            RynkHostError::TrailingBytes { .. } => "ResponseTrailingBytes",
            RynkHostError::CmdMismatch { .. } => "ResponseCommandMismatch",
            RynkHostError::TopicCmd(_) => "InvalidRequestCommand",
        };
        let err = js_sys::Error::new(&e.to_string());
        err.set_name(kind);
        err.into()
    }
}

/// Rynk client over any byte link implementing the embedded-io-async
/// [`Read`] + [`Write`] traits. See the crate docs for the transport contract.
/// [`next_event`](crate::Client::next_event) is always cancel-safe.
pub struct Client<T: Read + Write> {
    transport: T,
    /// Per-`read` scratch, separate from `rx_buf` so a cancelled read leaves no
    /// partial length behind.
    read_scratch: Box<[u8; READ_SCRATCH_SIZE]>,
    /// RX reassembly buffer.
    rx_buf: Vec<u8>,
    /// Request SEQ, cycling through `1..=255`.
    next_seq: u8,
    /// Set when the link is unrecoverable; every later call fails fast until the
    /// client is rebuilt.
    dead: bool,
    /// Set across the `write_all` in `send_request`, cleared once it completes.
    send_in_flight: bool,
    /// Queued topic frames.
    events: VecDeque<TopicFrame>,
    /// Topics dropped from a full queue.
    events_dropped: u64,
    /// Reusable TX scratch.
    tx_buf: Vec<u8>,
    /// Handshake capability snapshot. Until `connect` fills it, holds the
    /// protocol floor: all flags off, `max_payload_size` at the pre-handshake
    /// frame limit.
    capabilities: DeviceCapabilities,
}

impl<T: Read + Write> Client<T> {
    /// Build an unhandshaked client.
    pub(crate) fn new(transport: T) -> Self {
        Self {
            transport,
            read_scratch: Box::new([0u8; READ_SCRATCH_SIZE]),
            rx_buf: Vec::with_capacity(READ_SCRATCH_SIZE),
            next_seq: 1,
            dead: false,
            send_in_flight: false,
            events: VecDeque::new(),
            events_dropped: 0,
            tx_buf: vec![0u8; RYNK_MIN_BUFFER_SIZE],
            // Placeholder; `connect` overwrites it from the handshake.
            capabilities: DeviceCapabilities {
                max_payload_size: (RYNK_MIN_BUFFER_SIZE - RYNK_HEADER_SIZE) as u16,
                ..Default::default()
            },
        }
    }

    /// Largest frame either side may send: header + the device's advertised
    /// `max_payload_size` (the protocol floor until the handshake fills it).
    fn max_frame_size(&self) -> usize {
        RYNK_HEADER_SIZE + self.capabilities.max_payload_size as usize
    }

    /// Handshake: negotiate the version, then read and cache device capabilities.
    ///
    /// Rejects only a **major** version mismatch; any same-major minor connects.
    /// `&mut T` also implements the I/O traits, so `connect(&mut transport)`
    /// keeps the transport with the caller: after a `VersionMismatch` the
    /// `GetVersion` round trip has completed, leaving the stream clean to retry
    /// with a client built for the firmware's major.
    pub async fn connect(transport: T) -> Result<Self, RynkHostError> {
        let mut client = Self::new(transport);
        let version: ProtocolVersion = client.request_raw(Cmd::GetVersion, &()).await?;

        let supported = ProtocolVersion::CURRENT;
        if version.major != supported.major {
            return Err(RynkHostError::VersionMismatch {
                firmware_major: version.major,
                firmware_minor: version.minor,
                host_major: supported.major,
                host_max_minor: supported.minor,
            });
        }
        if version.minor > supported.minor {
            log::info!(
                "rynk: firmware protocol v{}.{} is newer than this client's v{}.{}; new commands/topics may be \
                 unavailable",
                version.major,
                version.minor,
                supported.major,
                supported.minor
            );
        }
        client.capabilities = client.request_raw(Cmd::GetCapabilities, &()).await?;
        // Grow TX scratch to the negotiated limit so any in-spec request encodes.
        let max_frame = client.max_frame_size();
        if max_frame > client.tx_buf.len() {
            client.tx_buf.resize(max_frame, 0);
        }
        Ok(client)
    }

    /// Cached capability snapshot from connect. Crate-internal: capability gating
    /// reads it; consumers read capabilities via the `get_capabilities` command.
    pub(crate) fn capabilities(&self) -> &DeviceCapabilities {
        &self.capabilities
    }

    /// The owned transport, e.g. to read connection identity.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// `false` once the link is dead — drop the client and reconnect.
    pub fn is_alive(&self) -> bool {
        !self.dead
    }

    /// Topics evicted from the in-client queue when it filled while
    /// [`next_event`](crate::Client::next_event) wasn't draining it.
    ///
    /// Counts in-client overflow only — not OS/BLE notification drops below the
    /// transport, which the client can't observe. Topics are best-effort (see
    /// [`IncomingTopic`](crate::IncomingTopic)); re-read with the matching `Get*` call when it matters.
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped
    }

    /// Drop any half-read frame from the RX buffer after a caller-owned timeout
    /// or cancellation. Does not revive a dead link.
    pub fn resync(&mut self) {
        self.rx_buf.clear();
    }

    /// Read the next topic push as a raw [`TopicFrame`] — the untyped
    /// counterpart of [`next_event`](crate::Client::next_event), paired with
    /// [`request_raw`](Self::request_raw) for topics with no typed `IncomingTopic`
    /// yet. Queued topics come first. Cancel-safe.
    pub async fn next_topic_frame(&mut self) -> Result<TopicFrame, RynkHostError> {
        if let Some(frame) = self.events.pop_front() {
            return Ok(frame);
        }
        if self.dead {
            return Err(RynkHostError::Disconnected);
        }
        loop {
            let (header, payload) = self.next_frame().await?;
            if header.cmd.is_topic() {
                return Ok(TopicFrame {
                    cmd: header.cmd,
                    payload,
                });
            }
            // Stale response.
        }
    }

    /// One request/response round trip, typed by the shared command table:
    /// `E` pins the `Cmd` and both payload types to the same definitions the
    /// firmware handlers compile against.
    pub async fn request<E: Endpoint>(&mut self, req: &E::Request) -> Result<E::Response, RynkHostError> {
        self.request_raw(E::CMD, req).await
    }

    /// Like [`request`](Self::request) but untyped — for experimental or future
    /// `Cmd`s the table doesn't carry yet. Unwraps the response envelope:
    /// `Ok(v)` on accept, `Err(RynkHostError::Rejected)` on device rejection.
    pub async fn request_raw<Req: Serialize, Resp: DeserializeOwned>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> Result<Resp, RynkHostError> {
        if cmd.is_topic() {
            return Err(RynkHostError::TopicCmd(cmd));
        }
        let seq = self.send_request(cmd, req).await?;

        loop {
            let (header, payload) = self.next_frame().await?;
            if header.cmd.is_topic() {
                if self.events.len() == EVENT_QUEUE_CAPACITY {
                    self.events.pop_front();
                    self.events_dropped += 1;
                    log::debug!(
                        "rynk: event queue full, dropped oldest topic ({} total)",
                        self.events_dropped
                    );
                }
                self.events.push_back(TopicFrame {
                    cmd: header.cmd,
                    payload,
                });
            } else if header.seq == seq {
                if header.cmd != cmd {
                    return Err(RynkHostError::CmdMismatch {
                        sent: cmd,
                        got: header.cmd,
                    });
                }
                // A response longer than its type means a wire mismatch; reject the tail.
                let (env, rest) = postcard::take_from_bytes::<Result<Resp, RynkError>>(&payload)
                    .map_err(|source| RynkHostError::Deserialize { cmd, source })?;
                if !rest.is_empty() {
                    return Err(RynkHostError::TrailingBytes { cmd });
                }
                return env.map_err(RynkHostError::Rejected);
            }
            // Stale response.
        }
    }

    /// Send one request frame without waiting for a reply — for commands whose
    /// effect prevents one (reboot, bootloader jump).
    pub async fn send_no_reply<E: Endpoint>(&mut self, req: &E::Request) -> Result<(), RynkHostError> {
        self.send_request(E::CMD, req).await.map(|_| ())
    }

    /// Encode one request into the TX scratch, write it, and return its SEQ
    /// (cycling `1..=255`).
    async fn send_request<Req: Serialize>(&mut self, cmd: Cmd, req: &Req) -> Result<u8, RynkHostError> {
        if self.dead || self.send_in_flight {
            // `send_in_flight` still set means that a prior send future was cancelled
            // mid-write, leaving a partial frame on the wire. Unrecoverable.
            self.dead = true;
            return Err(RynkHostError::Disconnected);
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.checked_add(1).unwrap_or(1);
        let frame_len = RynkMessage::build(&mut self.tx_buf, cmd, seq, req)
            .map_err(|_| RynkHostError::Encode(cmd))?
            .frame_len();
        let max = self.max_frame_size();
        if frame_len > max {
            return Err(RynkHostError::TooLarge { cmd, frame_len, max });
        }
        // Latch across the await: if this future is dropped mid-write, the flag
        // stays set and the next wire op marks the link dead.
        self.send_in_flight = true;
        let result = self.transport.write_all(&self.tx_buf[..frame_len]).await;
        self.send_in_flight = false;
        if let Err(e) = result {
            self.dead = true;
            return Err(RynkHostError::Io(format!("{e:?}")));
        }
        Ok(seq)
    }

    /// Read the next complete frame. A read failure or EOF marks the link
    /// dead — a partially read frame has no recovery point short of reconnect.
    async fn next_frame(&mut self) -> Result<(RynkHeader, Vec<u8>), RynkHostError> {
        if self.send_in_flight {
            // A prior send was cancelled mid-write; the stream is desynced.
            self.dead = true;
            return Err(RynkHostError::Disconnected);
        }
        loop {
            let header = self.rx_buf.first_chunk::<RYNK_HEADER_SIZE>().map(RynkHeader::parse);
            if let Some(header) = header {
                let frame_len = header.frame_len();

                // A conforming peer never exceeds its limit; an oversized header
                // means a desynced stream, so drop and re-sync.
                if frame_len > self.max_frame_size() {
                    log::debug!("rynk: oversized frame header, dropping {} bytes", self.rx_buf.len());
                    self.rx_buf.clear();
                    continue;
                }

                if self.rx_buf.len() >= frame_len {
                    let payload = self.rx_buf[RYNK_HEADER_SIZE..frame_len].to_vec();
                    self.rx_buf.drain(..frame_len);
                    return Ok((header, payload));
                }
            }

            // Over-reads are fine: pipelined frames accumulate in `rx_buf`.
            // Reading into scratch keeps a cancelled read from corrupting
            // `rx_buf` — nothing is appended until the read lands.
            let n = match self.transport.read(&mut self.read_scratch[..]).await {
                Ok(0) => {
                    self.dead = true;
                    return Err(RynkHostError::Disconnected);
                }
                Ok(n) => n,
                Err(e) => {
                    self.dead = true;
                    return Err(RynkHostError::Io(format!("{e:?}")));
                }
            };
            self.rx_buf.extend_from_slice(&self.read_scratch[..n]);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::VecDeque;
    use std::time::Duration;

    use embedded_io_async::ErrorKind;
    use rmk_types::action::KeyAction;
    use rmk_types::connection::{ConnectionStatus, ConnectionType};
    use rmk_types::protocol::rynk::{
        GetComboBulkResponse, GetKeymapBulkResponse, GetMorseBulkResponse, SetComboBulkRequest, SetKeymapBulkRequest,
        SetMorseBulkRequest, TopicEvent,
    };
    use tokio::time::timeout;

    use super::*;
    use crate::IncomingTopic;

    enum Step {
        Chunk(Vec<u8>),
        Hang,
    }

    /// Scripted byte link: each `Chunk` is delivered across one or more reads
    /// (partial reads handled by `pos`), `Hang` parks the reader, exhaustion
    /// reads EOF. Writes succeed unless `fail_send` is set.
    struct MockTransport {
        steps: VecDeque<Step>,
        pending: Vec<u8>,
        pos: usize,
        fail_send: bool,
        /// When set, `write` parks forever — lets a test cancel a send mid-write.
        hang_send: bool,
    }
    impl MockTransport {
        fn new(steps: Vec<Step>) -> Self {
            Self {
                steps: steps.into(),
                pending: Vec::new(),
                pos: 0,
                fail_send: false,
                hang_send: false,
            }
        }
    }
    impl embedded_io_async::ErrorType for MockTransport {
        type Error = ErrorKind;
    }
    impl embedded_io_async::Read for MockTransport {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ErrorKind> {
            while self.pos >= self.pending.len() {
                match self.steps.pop_front() {
                    Some(Step::Chunk(c)) => {
                        self.pending = c;
                        self.pos = 0;
                    }
                    Some(Step::Hang) => std::future::pending().await,
                    None => return Ok(0),
                }
            }
            let n = buf.len().min(self.pending.len() - self.pos);
            buf[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }
    impl embedded_io_async::Write for MockTransport {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, ErrorKind> {
            if self.hang_send {
                std::future::pending::<()>().await;
            }
            if self.fail_send {
                return Err(ErrorKind::Other);
            }
            Ok(buf.len())
        }

        async fn flush(&mut self) -> Result<(), ErrorKind> {
            Ok(())
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

    // Representative device; omitted fields take their zero/false default. Tests
    // that need a flag flip it on a local copy (e.g. `bulk_transfer_supported`).
    fn caps() -> DeviceCapabilities {
        DeviceCapabilities {
            num_layers: 4,
            num_rows: 6,
            num_cols: 14,
            max_combos: 8,
            max_combo_keys: 4,
            max_macros: 8,
            macro_space_size: 1024,
            max_morse: 4,
            max_patterns_per_key: 4,
            max_forks: 4,
            storage_enabled: true,
            max_payload_size: 256,
            macro_chunk_size: 64,
            ..Default::default()
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
        assert!(matches!(r, Err(RynkHostError::Rejected(RynkError::Invalid))));
    }

    #[tokio::test]
    async fn trailing_bytes_rejected() {
        // A u16 reply with extra bytes — response longer than the type.
        let mut chunk = reply(Cmd::GetWpm, 1, 42u16);
        chunk[3] += 2; // bump the declared LEN
        chunk.extend_from_slice(&[0xAA, 0xBB]);
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let r = c.get_wpm().await;
        assert!(matches!(r, Err(RynkHostError::TrailingBytes { cmd: Cmd::GetWpm })));
    }

    #[tokio::test]
    async fn topic_cmd_to_request_rejected() {
        let mut c = raw_client(vec![]);
        let r = c.request_raw::<(), u8>(Cmd::LayerChange, &()).await;
        assert!(matches!(r, Err(RynkHostError::TopicCmd(Cmd::LayerChange))));
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
        assert!(
            matches!(ev, IncomingTopic::Unknown(ref f) if f.cmd == Cmd::from_raw(0x80ff) && f.payload == [1, 2, 3])
        );
    }

    #[tokio::test]
    async fn unknown_response_cmd_mismatch_detected() {
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::from_raw(0x7fff), 1, 42u16))]);
        let r = c.get_wpm().await;
        assert!(matches!(
            r,
            Err(RynkHostError::CmdMismatch {
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
    async fn oversized_inbound_frame_dropped_then_resyncs() {
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
            Step::Chunk(header(Cmd::GetWpm.raw(), 3, u16::MAX)),
            Step::Chunk(reply(Cmd::GetWpm, 3, 42u16)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert_eq!(client.get_wpm().await.unwrap(), 42);
        assert!(client.is_alive(), "an oversized inbound frame is dropped, not fatal");
    }

    #[tokio::test]
    async fn link_death_fails_fast() {
        let mut c = raw_client(vec![]);
        let r1 = c.get_wpm().await;
        assert!(matches!(r1, Err(RynkHostError::Disconnected)));
        assert!(!c.is_alive());
        let r2 = c.get_wpm().await;
        assert!(matches!(r2, Err(RynkHostError::Disconnected)));
        let ev = c.next_event().await;
        assert!(matches!(ev, Err(RynkHostError::Disconnected)));
    }

    #[tokio::test]
    async fn send_failure_marks_link_dead() {
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::GetWpm, 1, 42u16))]);
        c.transport.fail_send = true;
        let r = c.get_wpm().await;
        assert!(matches!(r, Err(RynkHostError::Io(_))));
        assert!(!c.is_alive(), "a failed send is unrecoverable");
        // Even with a readable reply queued, the dead link fails fast.
        let r2 = c.get_wpm().await;
        assert!(matches!(r2, Err(RynkHostError::Disconnected)));
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_mid_send_latches_dead() {
        // A send that parks mid-write is cancelled by an external timeout. The
        // partial frame desynced the peer, so the next request must fail fast
        // instead of sending into a desynced stream while reporting healthy.
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::GetWpm, 1, 42u16))]);
        c.transport.hang_send = true;
        let cancelled = timeout(Duration::from_millis(10), c.get_wpm()).await;
        assert!(cancelled.is_err(), "the hung send must be cancelled by the timeout");
        assert!(
            c.is_alive(),
            "death is detected on the next op, not from the drop itself"
        );

        c.transport.hang_send = false;
        let r = c.get_wpm().await;
        assert!(matches!(r, Err(RynkHostError::Disconnected)));
        assert!(!c.is_alive(), "a cancelled mid-send must latch the link dead");
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_mid_send_then_next_event_fails_fast() {
        // The read path must also refuse the desynced stream: after a cancelled
        // mid-send, next_event (which sends nothing) latches dead rather than
        // reassembling garbage off a wedged link.
        let mut c = raw_client(vec![Step::Chunk(topic(Cmd::LayerChange, 3u8))]);
        c.transport.hang_send = true;
        let cancelled = timeout(Duration::from_millis(10), c.get_wpm()).await;
        assert!(cancelled.is_err());

        c.transport.hang_send = false;
        let ev = c.next_event().await;
        assert!(matches!(ev, Err(RynkHostError::Disconnected)));
        assert!(!c.is_alive());
    }

    #[tokio::test]
    async fn topic_during_request_is_queued() {
        let mut chunk = topic(Cmd::LayerChange, 3u8);
        chunk.extend_from_slice(&reply(Cmd::GetWpm, 1, 42u16));
        let mut c = raw_client(vec![Step::Chunk(chunk)]);
        let got = c.get_wpm().await.unwrap();
        assert_eq!(got, 42);
        let ev = c.next_event().await.unwrap();
        assert!(matches!(ev, IncomingTopic::Topic(TopicEvent::LayerChange(3))));
    }

    #[tokio::test]
    async fn next_event_reads_from_link() {
        let mut c = raw_client(vec![Step::Chunk(topic(Cmd::LayerChange, 7u8))]);
        let ev = c.next_event().await.unwrap();
        assert!(matches!(ev, IncomingTopic::Topic(TopicEvent::LayerChange(7))));
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
            Err(RynkHostError::CmdMismatch {
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
        assert!(matches!(ev, IncomingTopic::Topic(TopicEvent::LayerChange(7))));
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
        assert_eq!(client.get_wpm().await.unwrap(), 37);
    }

    #[tokio::test]
    async fn capability_gate_rejects_without_wire_send() {
        // caps() has ble_enabled = false, so a BLE-only call must reject locally
        // with `Unsupported`, consuming no transport step — the mock has none left,
        // so a wire send would surface `Disconnected`.
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert!(!client.capabilities().ble_enabled);
        let r = client.get_battery_status().await;
        assert!(matches!(r, Err(RynkHostError::Unsupported(Cmd::GetBatteryStatus, _))));
        assert!(client.is_alive(), "a locally-gated reject must not kill the link");
    }

    #[tokio::test]
    async fn oversized_request_rejected_locally() {
        // Firmware advertises a tiny max_payload_size, so a request whose encoded
        // frame exceeds it must be rejected locally with `TooLarge` — consuming no
        // transport step and without killing the link.
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
            Err(RynkHostError::TooLarge {
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
    async fn bulk_methods_gate_without_wire_send() {
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert!(!client.capabilities().bulk_transfer_supported);

        let keymap_req = SetKeymapBulkRequest {
            layer: 0,
            start_row: 0,
            start_col: 0,
            actions: Default::default(),
        };
        let combo_req = SetComboBulkRequest {
            start_index: 0,
            configs: Default::default(),
        };
        let morse_req = SetMorseBulkRequest {
            start_index: 0,
            configs: Default::default(),
        };

        assert!(matches!(
            client.get_keymap_bulk(0, 0, 0, 1).await,
            Err(RynkHostError::Unsupported(Cmd::GetKeymapBulk, _))
        ));
        assert!(matches!(
            client.set_keymap_bulk(keymap_req).await,
            Err(RynkHostError::Unsupported(Cmd::SetKeymapBulk, _))
        ));
        assert!(matches!(
            client.get_combo_bulk(0, 1).await,
            Err(RynkHostError::Unsupported(Cmd::GetComboBulk, _))
        ));
        assert!(matches!(
            client.set_combo_bulk(combo_req).await,
            Err(RynkHostError::Unsupported(Cmd::SetComboBulk, _))
        ));
        assert!(matches!(
            client.get_morse_bulk(0, 1).await,
            Err(RynkHostError::Unsupported(Cmd::GetMorseBulk, _))
        ));
        assert!(matches!(
            client.set_morse_bulk(morse_req).await,
            Err(RynkHostError::Unsupported(Cmd::SetMorseBulk, _))
        ));
        assert!(client.is_alive(), "locally-gated bulk rejects must not kill the link");
    }

    #[tokio::test]
    async fn bulk_methods_round_trip_when_supported() {
        let mut supported = caps();
        supported.bulk_transfer_supported = true;
        supported.max_bulk_keys = 8;

        let keymap_resp = GetKeymapBulkResponse {
            actions: Default::default(),
        };
        let combo_resp = GetComboBulkResponse {
            configs: Default::default(),
        };
        let morse_resp = GetMorseBulkResponse {
            configs: Default::default(),
        };
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, supported)),
            Step::Chunk(reply(Cmd::SetKeymapBulk, 3, ())),
            Step::Chunk(reply(Cmd::GetKeymapBulk, 4, keymap_resp.clone())),
            Step::Chunk(reply(Cmd::SetComboBulk, 5, ())),
            Step::Chunk(reply(Cmd::GetComboBulk, 6, combo_resp.clone())),
            Step::Chunk(reply(Cmd::SetMorseBulk, 7, ())),
            Step::Chunk(reply(Cmd::GetMorseBulk, 8, morse_resp.clone())),
        ]);
        let mut client = Client::connect(t).await.unwrap();

        client
            .set_keymap_bulk(SetKeymapBulkRequest {
                layer: 0,
                start_row: 0,
                start_col: 0,
                actions: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(client.get_keymap_bulk(0, 0, 0, 1).await.unwrap(), keymap_resp);

        client
            .set_combo_bulk(SetComboBulkRequest {
                start_index: 0,
                configs: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(client.get_combo_bulk(0, 1).await.unwrap(), combo_resp);

        client
            .set_morse_bulk(SetMorseBulkRequest {
                start_index: 0,
                configs: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(client.get_morse_bulk(0, 1).await.unwrap(), morse_resp);
    }

    #[tokio::test]
    async fn next_event_decodes_typed_payload() {
        // A ConnectionChange topic must decode into the typed IncomingTopic variant.
        let status = ConnectionStatus {
            preferred: ConnectionType::Ble,
            ..Default::default()
        };
        let mut c = raw_client(vec![Step::Chunk(topic(Cmd::ConnectionChange, status))]);
        let ev = c.next_event().await.unwrap();
        match ev {
            IncomingTopic::Topic(TopicEvent::ConnectionChange(s)) => assert_eq!(s.preferred, ConnectionType::Ble),
            other => panic!("expected ConnectionChange, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn next_event_undecodable_payload_is_unknown() {
        // A known topic cmd whose payload can't decode (LayerChange needs 1 byte,
        // here it's empty) surfaces as IncomingTopic::Unknown rather than being dropped.
        let mut c = raw_client(vec![Step::Chunk(header(Cmd::LayerChange.raw(), 0, 0))]);
        let ev = c.next_event().await.unwrap();
        assert!(matches!(ev, IncomingTopic::Unknown(ref f) if f.cmd == Cmd::LayerChange && f.payload.is_empty()));
    }

    #[tokio::test]
    async fn connect_rejects_newer_major() {
        let newer = ProtocolVersion {
            major: ProtocolVersion::CURRENT.major + 1,
            minor: 0,
        };
        let t = MockTransport::new(vec![Step::Chunk(reply(Cmd::GetVersion, 1, newer))]);
        let err = Client::connect(t).await.err().expect("connect must fail");
        assert!(matches!(err, RynkHostError::VersionMismatch { .. }));
    }

    #[tokio::test]
    async fn connect_accepts_newer_minor() {
        // Same major, newer minor: minor is informational, so connect must
        // succeed (rejection is major-only). get_version coverage lives in loopback.
        let newer = ProtocolVersion {
            major: ProtocolVersion::CURRENT.major,
            minor: ProtocolVersion::CURRENT.minor + 1,
        };
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, newer)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
        ]);
        Client::connect(t).await.expect("same-major newer-minor must connect");
    }

    #[tokio::test]
    async fn connect_retries_same_transport_after_version_mismatch() {
        // `&mut T` still implements `Read + Write` (embedded-io blanket impls),
        // so the caller keeps the transport across a VersionMismatch and can
        // retry — the failed probe's round trip completed, leaving the stream
        // clean. The retry's client restarts SEQ at 1.
        let newer_major = ProtocolVersion {
            major: ProtocolVersion::CURRENT.major + 1,
            minor: 0,
        };
        let mut t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, newer_major)),
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
        ]);
        let err = Client::connect(&mut t).await.err().expect("newer major must mismatch");
        assert!(matches!(err, RynkHostError::VersionMismatch { .. }));
        Client::connect(&mut t).await.expect("retry over the same transport");
    }

    #[tokio::test(start_paused = true)]
    async fn caller_can_timeout_silent_connect() {
        let t = MockTransport::new(vec![Step::Hang]);
        let err = timeout(Duration::from_millis(10), Client::connect(t)).await;
        assert!(err.is_err());
    }

    // ── RynkDevice trait wiring (hardware-free) ──

    #[tokio::test]
    async fn rynk_device_trait_drives_lifecycle() {
        use crate::RynkDevice;

        // A device whose `open` hands back a scripted transport (handshake + one
        // reply). Proves a generic `D: RynkDevice` consumer drives connect →
        // request without naming the transport; discovery is the transport's own
        // inherent call, not part of the trait.
        struct MockDevice;
        impl MockDevice {
            async fn discover() -> Result<Vec<Self>, RynkHostError> {
                Ok(vec![MockDevice])
            }
        }
        impl RynkDevice for MockDevice {
            type Transport = MockTransport;
            fn label(&self) -> String {
                "mock".into()
            }
            async fn open(self) -> Result<MockTransport, RynkHostError> {
                Ok(MockTransport::new(vec![
                    Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
                    Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
                    Step::Chunk(reply(Cmd::GetWpm, 3, 7u16)),
                ]))
            }
        }

        async fn drive<D: RynkDevice>(d: D) -> u16 {
            assert_eq!(d.label(), "mock");
            let mut client = d.connect().await.unwrap();
            client.get_wpm().await.unwrap()
        }

        let devices = MockDevice::discover().await.unwrap();
        let device = devices.into_iter().next().unwrap();
        assert_eq!(drive(device).await, 7);
    }
}
