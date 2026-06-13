//! Protocol driver: framing, SEQ correlation, topic queueing, link lifecycle.
//!
//! This layer is version-independent by design — it is what splits out into
//! `rynk-core` when multi-version support lands. The typed, version-specific
//! API surface (endpoint methods, [`Event`](crate::Event) decoding) lives in
//! `api.rs` as a second impl block on [`Client`].

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

/// Queued topic frames before dropping the oldest.
const EVENT_QUEUE_CAPACITY: usize = 64;

/// RX scratch size per `read` call; bigger frames accumulate across reads.
const READ_SCRATCH_SIZE: usize = 4096;

/// Transport-level failures, as the client reports them.
///
/// The I/O traits' own error types are transport-specific; the client folds
/// them into this one currency (`Ok(0)` reads become [`Disconnected`](Self::Disconnected),
/// errors become [`Io`](Self::Io)). [`DeviceNotFound`](Self::DeviceNotFound)
/// is produced by the transport crates' discovery helpers.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport disconnected")]
    Disconnected,
    #[error("io error: {0}")]
    Io(String),
    #[error("device not found: {0}")]
    DeviceNotFound(String),
}

/// A raw topic frame (server → host push), delivered via
/// [`Client::next_event`](crate::Client::next_event).
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
    /// Candidate devices were found, but none answered the handshake.
    #[error("{probed} candidate device(s) found, none answered the Rynk handshake (last: {last})")]
    NoResponsiveDevice { probed: usize, last: Box<ConnectError> },
    /// Fires on a protocol **major** mismatch only. Connect via
    /// `&mut transport` to retry the same link with a client built for the
    /// firmware's major.
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
}

/// Rynk client over any byte link implementing the embedded-io-async
/// [`Read`] + [`Write`] traits — the same seam the firmware session uses.
/// See the crate docs for the transport-implementer contract.
///
/// Requests are cancel-safe once the send completes — cancelling a request
/// future mid-send can leave a partial frame and desync the device until
/// reconnect. [`next_event`](crate::Client::next_event) is always cancel-safe.
pub struct Client<T: Read + Write> {
    transport: T,
    /// Fixed RX scratch one `read` fills per call; kept out of `rx_buf` so a
    /// cancelled read never leaves uninitialized length behind.
    read_scratch: Box<[u8; READ_SCRATCH_SIZE]>,
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
    /// Capability snapshot from the handshake. Until `connect` overwrites it,
    /// holds the protocol floor: every flag off, `max_payload_size` at the
    /// pre-handshake frame limit.
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
            events: VecDeque::new(),
            events_dropped: 0,
            tx_buf: vec![0u8; RYNK_MIN_BUFFER_SIZE],
            // Construction-only placeholders; `connect` overwrites both with
            // the handshake values before handing the client out.
            protocol_version: ProtocolVersion::CURRENT,
            capabilities: DeviceCapabilities {
                max_payload_size: (RYNK_MIN_BUFFER_SIZE - RYNK_HEADER_SIZE) as u16,
                ..Default::default()
            },
        }
    }

    /// Largest frame either side may put on the wire — header + the device's
    /// advertised max_payload_size (the protocol floor before the handshake).
    fn max_frame_size(&self) -> usize {
        RYNK_HEADER_SIZE + self.capabilities.max_payload_size as usize
    }

    /// Handshake and read device capabilities.
    ///
    /// Rejects only a protocol **major** mismatch; same-major firmware of any
    /// minor connects (the ICD keeps same-major changes wire-compatible).
    /// `&mut T` implements the I/O traits too (embedded-io blanket impls), so
    /// `connect(&mut transport)` keeps the transport with the caller — after a
    /// `VersionMismatch` the `GetVersion` round trip has completed, leaving
    /// the stream clean for a retry with a client built for the firmware's
    /// major.
    pub async fn connect(transport: T) -> Result<Self, ConnectError> {
        let mut client = Self::new(transport);
        let version: ProtocolVersion = client.request_raw(Cmd::GetVersion, &()).await?;

        let supported = ProtocolVersion::CURRENT;
        if version.major != supported.major {
            return Err(ConnectError::VersionMismatch {
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
        client.protocol_version = version;

        client.capabilities = client.request_raw(Cmd::GetCapabilities, &()).await?;
        // Grow the TX scratch to the negotiated limit so any in-spec request
        // (e.g. a bulk transfer via `request_raw`) encodes.
        let max_frame = client.max_frame_size();
        if max_frame > client.tx_buf.len() {
            client.tx_buf.resize(max_frame, 0);
        }
        Ok(client)
    }

    /// Cached capability snapshot from connect time.
    pub fn capabilities(&self) -> &DeviceCapabilities {
        &self.capabilities
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
    /// while no consumer was draining [`next_event`](crate::Client::next_event).
    ///
    /// Counts **only** in-client queue overflow — not OS/BLE-level notification
    /// drops below the transport, which the client cannot observe. Treat topics
    /// as best-effort (see [`Event`](crate::Event)) and re-read current values
    /// with the matching `Get*` call when they matter.
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped
    }

    /// Clear the RX reassembly buffer after a caller-owned timeout or other
    /// external cancellation point. This does not reopen a dead link.
    pub fn resync(&mut self) {
        self.rx_buf.clear();
    }

    /// Read the next topic push as a raw [`TopicFrame`] — the untyped
    /// counterpart of [`next_event`](crate::Client::next_event), paired with
    /// [`request_raw`](Self::request_raw) for topics without a typed `Event`
    /// yet. Queued topics are returned first. Cancel-safe.
    pub async fn next_topic_frame(&mut self) -> Result<TopicFrame, TransportError> {
        if let Some(frame) = self.events.pop_front() {
            return Ok(frame);
        }
        if self.dead {
            return Err(TransportError::Disconnected);
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

    /// One request/response round-trip, typed by the shared command table:
    /// `E` pins the `Cmd` and both payload types to the same definitions
    /// the firmware handlers compile against.
    pub async fn request<E: Endpoint>(&mut self, req: &E::Request) -> Result<E::Response, RequestError> {
        self.request_raw(E::CMD, req).await
    }

    /// One request/response round-trip with the response envelope unwrapped:
    /// `Ok(v)` on accept, `Err(RequestError::Rejected)` on device rejection.
    /// The typed methods are thin wrappers over [`request`](Self::request);
    /// call this directly for experimental or future `Cmd`s the table doesn't
    /// carry yet.
    pub async fn request_raw<Req: Serialize, Resp: DeserializeOwned>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> Result<Resp, RequestError> {
        if cmd.is_topic() {
            return Err(RequestError::TopicCmd(cmd));
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
                    return Err(RequestError::CmdMismatch {
                        sent: cmd,
                        got: header.cmd,
                    });
                }
                // A response longer than its type is a wire/type mismatch (a
                // major bump per the ICD), so reject a non-empty tail.
                let (env, rest) = postcard::take_from_bytes::<Result<Resp, RynkError>>(&payload)
                    .map_err(|source| RequestError::Deserialize { cmd, source })?;
                if !rest.is_empty() {
                    return Err(RequestError::TrailingBytes { cmd });
                }
                return env.map_err(RequestError::Rejected);
            }
            // Stale response.
        }
    }

    /// Send one request frame without waiting for a reply — for commands whose
    /// effect prevents one (reboot, bootloader jump).
    pub async fn send_no_reply<E: Endpoint>(&mut self, req: &E::Request) -> Result<(), RequestError> {
        self.send_request(E::CMD, req).await.map(|_| ())
    }

    /// Encode one request frame into the TX scratch and write it to the link;
    /// returns its SEQ (cycling `1..=255`). A failed send leaves a partial
    /// frame that desyncs the device, so the link is marked dead. No flush:
    /// `write_all` hands the frame to the transport whole, and the shipped
    /// transports deliver on write (a serial flush would be `tcdrain` — a
    /// blocking wait on a wedged device).
    async fn send_request<Req: Serialize>(&mut self, cmd: Cmd, req: &Req) -> Result<u8, RequestError> {
        if self.dead {
            return Err(TransportError::Disconnected.into());
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.checked_add(1).unwrap_or(1);
        let frame_len = RynkMessage::build(&mut self.tx_buf, cmd, seq, req)
            .map_err(|_| RequestError::Encode(cmd))?
            .frame_len();
        let max = self.max_frame_size();
        if frame_len > max {
            return Err(RequestError::TooLarge { cmd, frame_len, max });
        }
        if let Err(e) = self.transport.write_all(&self.tx_buf[..frame_len]).await {
            self.dead = true;
            return Err(TransportError::Io(format!("{e:?}")).into());
        }
        Ok(seq)
    }

    /// Read the next complete frame. A read failure or EOF marks the link
    /// dead — a partially read frame has no recovery point short of reconnect.
    async fn next_frame(&mut self) -> Result<(RynkHeader, Vec<u8>), TransportError> {
        loop {
            let header = self.rx_buf.first_chunk::<RYNK_HEADER_SIZE>().map(RynkHeader::parse);
            if let Some(header) = header {
                let frame_len = header.frame_len();

                // A conforming peer never exceeds its advertised limit, so an
                // oversized header means a corrupt/desynced stream: drop and re-sync.
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

            // Reading beyond the current frame is fine — pipelined frames
            // accumulate in `rx_buf`. The scratch indirection keeps a cancelled
            // read from corrupting `rx_buf` (nothing is appended until the read
            // lands).
            let n = match self.transport.read(&mut self.read_scratch[..]).await {
                Ok(0) => {
                    self.dead = true;
                    return Err(TransportError::Disconnected);
                }
                Ok(n) => n,
                Err(e) => {
                    self.dead = true;
                    return Err(TransportError::Io(format!("{e:?}")));
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
        SetMorseBulkRequest,
    };
    use tokio::time::timeout;

    use super::*;
    use crate::Event;

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
    }
    impl MockTransport {
        fn new(steps: Vec<Step>) -> Self {
            Self {
                steps: steps.into(),
                pending: Vec::new(),
                pos: 0,
                fail_send: false,
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
            Err(RequestError::Unsupported(Cmd::GetKeymapBulk, _))
        ));
        assert!(matches!(
            client.set_keymap_bulk(keymap_req).await,
            Err(RequestError::Unsupported(Cmd::SetKeymapBulk, _))
        ));
        assert!(matches!(
            client.get_combo_bulk(0, 1).await,
            Err(RequestError::Unsupported(Cmd::GetComboBulk, _))
        ));
        assert!(matches!(
            client.set_combo_bulk(combo_req).await,
            Err(RequestError::Unsupported(Cmd::SetComboBulk, _))
        ));
        assert!(matches!(
            client.get_morse_bulk(0, 1).await,
            Err(RequestError::Unsupported(Cmd::GetMorseBulk, _))
        ));
        assert!(matches!(
            client.set_morse_bulk(morse_req).await,
            Err(RequestError::Unsupported(Cmd::SetMorseBulk, _))
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

    #[tokio::test]
    async fn connect_accepts_newer_minor() {
        // Same major, newer minor: minor is informational, the connect must
        // succeed and report the firmware's version.
        let newer = ProtocolVersion {
            major: ProtocolVersion::CURRENT.major,
            minor: ProtocolVersion::CURRENT.minor + 1,
        };
        let t = MockTransport::new(vec![
            Step::Chunk(reply(Cmd::GetVersion, 1, newer)),
            Step::Chunk(reply(Cmd::GetCapabilities, 2, caps())),
        ]);
        let client = Client::connect(t).await.expect("same-major newer-minor must connect");
        assert_eq!(client.protocol_version(), newer);
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
        assert!(matches!(err, ConnectError::VersionMismatch { .. }));
        let client = Client::connect(&mut t).await.expect("retry over the same transport");
        assert_eq!(client.protocol_version(), ProtocolVersion::CURRENT);
    }

    #[tokio::test(start_paused = true)]
    async fn caller_can_timeout_silent_connect() {
        let t = MockTransport::new(vec![Step::Hang]);
        let err = timeout(Duration::from_millis(10), Client::connect(t)).await;
        assert!(err.is_err());
    }
}
