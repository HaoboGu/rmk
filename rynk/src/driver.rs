//! Protocol driver for Rynk: framing, SEQ correlation, topic queueing, link lifecycle.
//!
//! [`Client`] handles transport framing, correlation, and demux. The typed
//! command surface lives in `api.rs`.
//!
//! ## Frame routing
//!
//! A frame is a 5-byte header (`CMD u16 | SEQ u8 | LEN u16`) plus a postcard
//! payload. The inbound stream routes by CMD and SEQ:
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
//! - Cancelling a read makes the stream boundary unknowable; the next operation
//!   latches the link dead and requires reconnecting.
//! - Broken links latch dead so later calls fail fast.

use std::collections::VecDeque;

use embedded_io_async::{Read, Write};
use rmk_types::protocol::rynk::endpoint::Endpoint;
use rmk_types::protocol::rynk::{
    Cmd, DeviceCapabilities, ProtocolVersion, RYNK_HEADER_SIZE, RynkError, RynkHeader, RynkMessage,
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

    /// Firmware accepted the request but answered with an error.
    #[error("device rejected {0:?}")]
    Rejected(RynkError),
    /// The request did not fit the device's advertised buffer, or failed to
    /// encode. The send scratch is sized to the device's `max_payload_size`.
    #[error("request {0:?} does not fit the device buffer (or failed to encode)")]
    Encode(Cmd),
    #[error("response decode failed for {cmd:?}: {source}")]
    Deserialize { cmd: Cmd, source: postcard::Error },
    /// `GetLayout` blob inflate or decode failed.
    #[error("layout blob decode failed: {0}")]
    Layout(String),
    #[error("response for {cmd:?} had trailing bytes")]
    TrailingBytes { cmd: Cmd },
    #[error("response cmd mismatch: sent {sent:?}, got {got:?}")]
    CmdMismatch { sent: Cmd, got: Cmd },
    /// A topic-range `Cmd` was passed to a request method.
    #[error("{0:?} is a topic, not a request")]
    TopicCmd(Cmd),
    /// Capabilities reject the command before touching the wire.
    #[error("device does not support {0:?}: {1}")]
    Unsupported(Cmd, &'static str),
}

/// Bridge host errors into JS errors with stable `name` values.
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
/// Cancelling an in-progress read invalidates the stream; reconnect before
/// issuing another operation.
pub struct Client<T: Read + Write> {
    transport: T,
    /// Per-read scratch; committed to `rx_buf` only after `read` returns.
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
    /// Set while assembling an inbound frame. If the future is cancelled, the
    /// flag remains set so the next operation rejects the desynchronized link.
    receive_in_flight: bool,
    /// Queued topic frames.
    events: VecDeque<TopicFrame>,
    /// Topics dropped from a full queue.
    events_dropped: u64,
    /// Reusable TX scratch.
    tx_buf: Vec<u8>,
    /// Device capabilities from the handshake. `Default` (all-zero) until
    /// `connect` fills it; only `max_payload_size` is read before then, and only
    /// to bound outbound frames — the host reads inbound frames unbounded.
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
            receive_in_flight: false,
            events: VecDeque::new(),
            events_dropped: 0,
            // Handshake requests carry no payload; grow after `GetCapabilities`.
            tx_buf: vec![0u8; RYNK_HEADER_SIZE],
            capabilities: DeviceCapabilities::default(),
        }
    }

    /// Largest frame the host may send: header + the device's advertised
    /// `max_payload_size`. Zero until the handshake fills it, which still admits
    /// the empty-payload handshake requests.
    fn max_frame_size(&self) -> usize {
        RYNK_HEADER_SIZE + self.capabilities.max_payload_size as usize
    }

    /// Handshake: negotiate the version, then read and cache device capabilities.
    ///
    /// Rejects only major-version mismatches; same-major minors connect.
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
        // Grow TX scratch to the negotiated frame limit.
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

    /// `false` once the link is unusable. A cancelled read or write poisons the
    /// client — this reads `false` immediately, before the next call latches `dead`.
    pub fn is_alive(&self) -> bool {
        !self.dead && !self.send_in_flight && !self.receive_in_flight
    }

    /// Topics evicted while [`next_event`](crate::Client::next_event) lagged.
    ///
    /// Counts only overflow the client can observe; re-read critical state.
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped
    }

    /// Read the next topic push as a raw [`TopicFrame`]. Queued topics come first.
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
            // Drop stale responses.
        }
    }

    /// One typed request/response round trip from the shared command table.
    pub async fn request<E: Endpoint>(&mut self, req: &E::Request) -> Result<E::Response, RynkHostError> {
        self.request_raw(E::CMD, req).await
    }

    /// Untyped request/response for commands not in the typed table yet.
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
                // Trailing bytes signal a wire/type mismatch.
                let (env, rest) = postcard::take_from_bytes::<Result<Resp, RynkError>>(&payload)
                    .map_err(|source| RynkHostError::Deserialize { cmd, source })?;
                if !rest.is_empty() {
                    return Err(RynkHostError::TrailingBytes { cmd });
                }
                return env.map_err(RynkHostError::Rejected);
            }
            // Drop stale responses.
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
        if self.dead || self.send_in_flight || self.receive_in_flight {
            // A cancelled wire operation leaves the stream desynced.
            self.dead = true;
            return Err(RynkHostError::Disconnected);
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.checked_add(1).unwrap_or(1);
        // The scratch is sized to the device's advertised buffer, so a request
        // too large for the device fails to encode here — a local reject that
        // leaves the link intact.
        let msg = RynkMessage::build(&mut self.tx_buf, cmd, seq, req).map_err(|_| RynkHostError::Encode(cmd))?;
        // If dropped mid-write, the next wire op marks the link dead.
        self.send_in_flight = true;
        let result = self.transport.write_all(msg.frame()).await;
        self.send_in_flight = false;
        if let Err(e) = result {
            self.dead = true;
            return Err(RynkHostError::Io(format!("{e:?}")));
        }
        Ok(seq)
    }

    /// Read the next complete frame; read failures mark the link dead.
    async fn next_frame(&mut self) -> Result<(RynkHeader, Vec<u8>), RynkHostError> {
        if self.send_in_flight || self.receive_in_flight {
            // A prior wire operation was cancelled.
            self.dead = true;
            return Err(RynkHostError::Disconnected);
        }
        self.receive_in_flight = true;
        let result = self.next_frame_inner().await;
        self.receive_in_flight = false;
        result
    }

    async fn next_frame_inner(&mut self) -> Result<(RynkHeader, Vec<u8>), RynkHostError> {
        loop {
            let header = self.rx_buf.first_chunk::<RYNK_HEADER_SIZE>().map(RynkHeader::parse);
            if let Some(header) = header {
                // Frames are read by their declared length and routed; the host
                // never rejects one by size. Replies are correlated by SEQ and
                // decoded; topics are best-effort (decode-or-Unknown). The u16
                // LEN already caps a single frame at ~64 KB.
                let frame_len = header.frame_len();
                if self.rx_buf.len() >= frame_len {
                    let payload = self.rx_buf[RYNK_HEADER_SIZE..frame_len].to_vec();
                    self.rx_buf.drain(..frame_len);
                    return Ok((header, payload));
                }
            }

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
    use rmk_types::battery::BatteryStatus;
    use rmk_types::connection::{ConnectionStatus, ConnectionType};
    use rmk_types::protocol::rynk::{
        GetComboBulkResponse, GetKeymapBulkResponse, GetMorseBulkResponse, PeripheralStatus, SetComboBulkRequest,
        SetKeymapBulkRequest, SetMorseBulkRequest, TopicEvent,
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
        // Skip the handshake but give the client a generous send budget and a
        // scratch sized to it, so request-sending tests work without a `connect`.
        // Other capabilities stay at their defaults, as before.
        let mut c = Client::new(MockTransport::new(steps));
        c.capabilities.max_payload_size = 4096;
        c.tx_buf.resize(c.max_frame_size(), 0);
        c
    }

    /// A bare frame: header + postcard(value). Used for raw headers and topics.
    fn frame<T: Serialize>(cmd: Cmd, seq: u8, value: &T) -> Vec<u8> {
        let mut buf = vec![0u8; READ_SCRATCH_SIZE];
        let msg = RynkMessage::build(&mut buf, cmd, seq, value).unwrap();
        msg.frame().to_vec()
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

    // Tests clone this and flip only the capability under test.
    fn caps() -> DeviceCapabilities {
        DeviceCapabilities {
            num_layers: 4,
            num_rows: 6,
            num_cols: 14,
            max_combos: 8,
            max_combo_keys: 4,
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
        // Reply declares extra bytes beyond the decoded u16.
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
    async fn cancelled_mid_read_latches_link_dead() {
        let mut c = raw_client(vec![Step::Chunk(header(Cmd::GetWpm.raw(), 0xEE, 100)), Step::Hang]);
        let r1 = timeout(Duration::from_millis(10), c.get_wpm()).await;
        assert!(r1.is_err());
        assert!(!c.is_alive(), "a cancelled read poisons the link");
        let r2 = c.get_wpm().await;
        assert!(matches!(r2, Err(RynkHostError::Disconnected)));
        assert!(!c.is_alive());
    }

    #[tokio::test]
    async fn oversized_inbound_topic_is_read_not_fatal() {
        // A topic larger than the negotiated max_payload_size (256) is no longer
        // rejected by size: the host reads it by length and surfaces it, link intact.
        let mut big = header(0x80ff, 0, 300);
        big.extend_from_slice(&vec![0xAB; 300]);
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
            Step::Chunk(big),
            Step::Chunk(reply(Cmd::GetWpm, 3, 7u16)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        let ev = client.next_event().await.unwrap();
        assert!(
            matches!(ev, IncomingTopic::Unknown(ref f) if f.cmd == Cmd::from_raw(0x80ff) && f.payload.len() == 300)
        );
        assert!(client.is_alive());
        assert_eq!(client.get_wpm().await.unwrap(), 7);
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
        // Queued bytes do not revive a dead link.
        let r2 = c.get_wpm().await;
        assert!(matches!(r2, Err(RynkHostError::Disconnected)));
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_mid_send_latches_dead() {
        // A cancelled mid-write desyncs the peer; the next request must fail fast.
        let mut c = raw_client(vec![Step::Chunk(reply(Cmd::GetWpm, 1, 42u16))]);
        c.transport.hang_send = true;
        let cancelled = timeout(Duration::from_millis(10), c.get_wpm()).await;
        assert!(cancelled.is_err(), "the hung send must be cancelled by the timeout");
        assert!(!c.is_alive(), "a cancelled send poisons the link");

        c.transport.hang_send = false;
        let r = c.get_wpm().await;
        assert!(matches!(r, Err(RynkHostError::Disconnected)));
        assert!(!c.is_alive(), "a cancelled mid-send must latch the link dead");
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_mid_send_then_next_event_fails_fast() {
        // Read-only APIs must also fail after a cancelled send.
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

    #[tokio::test(start_paused = true)]
    async fn caller_cancel_mid_reply_wait_latches_link_dead() {
        let mut c = raw_client(vec![Step::Hang]);
        let cancelled = timeout(Duration::from_millis(10), c.get_wpm()).await;
        assert!(cancelled.is_err(), "outer timeout cancels request 1 mid-wait");
        assert!(!c.is_alive(), "the cancelled read poisons the link immediately");
        assert!(matches!(c.get_wpm().await, Err(RynkHostError::Disconnected)));
        assert!(!c.is_alive());
    }

    #[tokio::test(start_paused = true)]
    async fn caller_cancel_next_event_mid_reassembly_latches_link_dead() {
        let mut c = raw_client(vec![
            Step::Chunk(header(Cmd::LayerChange.raw(), 0, 1)), // topic header, payload pending
            Step::Hang,
        ]);
        let cancelled = timeout(Duration::from_millis(10), c.next_event()).await;
        assert!(cancelled.is_err());
        assert!(!c.is_alive(), "a cancelled next_event poisons the link");
        assert!(matches!(c.get_wpm().await, Err(RynkHostError::Disconnected)));
        assert!(!c.is_alive());
    }

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
        // The mock has no BLE reply; any wire send would disconnect.
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
    async fn wired_split_peripheral_status_is_supported() {
        let status = PeripheralStatus {
            connected: true,
            battery: BatteryStatus::Unavailable,
        };
        let mut capabilities = caps();
        capabilities.is_split = true;
        capabilities.ble_enabled = false;
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, capabilities)),
            Step::Chunk(reply(Cmd::GetPeripheralStatus, 3, status)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert_eq!(client.get_peripheral_status(0).await.unwrap(), status);
    }

    #[tokio::test]
    async fn oversized_request_rejected_locally() {
        // Too-large requests are rejected locally without killing the link.
        let mut tiny = caps();
        tiny.max_payload_size = 4;
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, tiny)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        let r = client.set_key(0, 0, 0, KeyAction::Morse(3)).await;
        assert!(matches!(r, Err(RynkHostError::Encode(Cmd::SetKeyAction))));
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
            client.get_keymap_bulk(0, 0, 0).await,
            Err(RynkHostError::Unsupported(Cmd::GetKeymapBulk, _))
        ));
        assert!(matches!(
            client.set_keymap_bulk(keymap_req).await,
            Err(RynkHostError::Unsupported(Cmd::SetKeymapBulk, _))
        ));
        assert!(matches!(
            client.get_combo_bulk(0).await,
            Err(RynkHostError::Unsupported(Cmd::GetComboBulk, _))
        ));
        assert!(matches!(
            client.set_combo_bulk(combo_req).await,
            Err(RynkHostError::Unsupported(Cmd::SetComboBulk, _))
        ));
        assert!(matches!(
            client.get_morse_bulk(0).await,
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
        assert_eq!(client.get_keymap_bulk(0, 0, 0).await.unwrap(), keymap_resp);

        client
            .set_combo_bulk(SetComboBulkRequest {
                start_index: 0,
                configs: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(client.get_combo_bulk(0).await.unwrap(), combo_resp);

        client
            .set_morse_bulk(SetMorseBulkRequest {
                start_index: 0,
                configs: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(client.get_morse_bulk(0).await.unwrap(), morse_resp);
    }

    #[tokio::test]
    async fn read_all_keymap_concatenates_pages() {
        // Ten keys at four per page yields 4, 4, then 2.
        let mut supported = caps();
        supported.bulk_transfer_supported = true;
        supported.max_bulk_keys = 4;
        supported.num_layers = 1;
        supported.num_rows = 1;
        supported.num_cols = 10;

        let page = |base: u8, n: u8| GetKeymapBulkResponse {
            actions: (0..n).map(|i| KeyAction::Morse(base + i)).collect(),
        };
        let expected: Vec<KeyAction> = (0u8..10).map(KeyAction::Morse).collect();

        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, supported)),
            Step::Chunk(reply(Cmd::GetKeymapBulk, 3, page(0, 4))),
            Step::Chunk(reply(Cmd::GetKeymapBulk, 4, page(4, 4))),
            Step::Chunk(reply(Cmd::GetKeymapBulk, 5, page(8, 2))),
            Step::Chunk(reply(Cmd::GetWpm, 6, 42u16)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert_eq!(client.read_all_keymap().await.unwrap(), expected);
        // Trailing reply proves the pager stopped after three fetches.
        assert_eq!(client.get_wpm().await.unwrap(), 42);
    }

    #[tokio::test]
    async fn read_all_stops_on_clamped_empty_page() {
        // An empty clamped page stops the read before `total`.
        let mut supported = caps();
        supported.bulk_transfer_supported = true;
        supported.max_bulk_keys = 4;
        supported.num_layers = 1;
        supported.num_rows = 1;
        supported.num_cols = 10;

        let full = GetKeymapBulkResponse {
            actions: (0u8..4).map(KeyAction::Morse).collect(),
        };
        let empty = GetKeymapBulkResponse { actions: vec![] };
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, supported)),
            Step::Chunk(reply(Cmd::GetKeymapBulk, 3, full)),
            Step::Chunk(reply(Cmd::GetKeymapBulk, 4, empty)),
            Step::Chunk(reply(Cmd::GetWpm, 5, 7u16)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        assert_eq!(client.read_all_keymap().await.unwrap().len(), 4);
        // Trailing reply proves the empty page halted the loop.
        assert_eq!(client.get_wpm().await.unwrap(), 7);
    }

    #[tokio::test]
    async fn write_all_keymap_chunks_by_page_size() {
        // Five actions at two per page should send 2, 2, then 1.
        let mut supported = caps();
        supported.bulk_transfer_supported = true;
        supported.max_bulk_keys = 2;

        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, ProtocolVersion::CURRENT)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, supported)),
            Step::Chunk(reply(Cmd::SetKeymapBulk, 3, ())),
            Step::Chunk(reply(Cmd::SetKeymapBulk, 4, ())),
            Step::Chunk(reply(Cmd::SetKeymapBulk, 5, ())),
            Step::Chunk(reply(Cmd::GetWpm, 6, 99u16)),
        ]);
        let mut client = Client::connect(t).await.unwrap();
        let actions: Vec<KeyAction> = (0u8..5).map(KeyAction::Morse).collect();
        client.write_all_keymap(&actions).await.unwrap();
        assert_eq!(client.get_wpm().await.unwrap(), 99);
    }

    #[tokio::test]
    async fn next_event_decodes_typed_payload() {
        // Known topic payloads decode into typed events.
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
        // Undecodable known topics stay visible as Unknown.
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
        // Minor version is informational within the same major.
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
        // The failed version probe consumes a clean round trip, so retry can reuse `&mut T`.
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

    #[tokio::test]
    async fn rynk_device_trait_drives_lifecycle() {
        use crate::RynkDevice;

        // Generic `RynkDevice` consumers should not name the transport type.
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
