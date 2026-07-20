//! Protocol driver for Rynk: the [`Client`] request/topic surface and the
//! [`Driver`] byte pump. Sessions are created by
//! [`RynkDevice::connect`](crate::RynkDevice::connect).
//!
//! ## Frame flow
//!
//! A frame is a 5-byte header (`CMD u16 | SEQ u8 | LEN u16`) plus a postcard
//! payload. Frames cross between [`Client`] and [`Driver`] as plain owned
//! bytes over three channels; [`RynkMessage`] is the view used to build and
//! parse them at each end:
//!
//! ```text
//! request()    encode → message ─→ Driver: write_all
//! Driver: read → reassemble → route by the CMD topic bit:
//!          topic frame → topics ─→ next_topic(): decode
//!          reply frame → resp   ─→ request(): SEQ match + decode
//! ```
//!
//! ## Session lifecycle
//!
//! [`Driver::run`] returns when the link dies; there is no in-band death
//! signal. Run it in the same `select` as everything awaiting on the
//! [`Client`] — see the crate docs for the usage topologies.

#[cfg(feature = "alloc")]
use alloc::{string::String, vec, vec::Vec};
use core::sync::atomic::{AtomicU8, Ordering};

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, TrySendError};
use embassy_sync::signal::Signal;
use embedded_io_async::{Error as _, ErrorKind, Read, Write};
use rmk_types::protocol::rynk::endpoint::Endpoint;
use rmk_types::protocol::rynk::{
    Cmd, DeviceCapabilities, ProtocolVersion, RYNK_HEADER_SIZE, RynkError, RynkHeader, RynkMessage, TopicEvent, command,
};
use serde::Serialize;
use thiserror::Error;

type CS = CriticalSectionRawMutex;

/// One whole frame (header + payload) as owned bytes. The no-alloc bound is
/// the firmware's own full-frame buffer size, so any frame it can send fits.
#[cfg(feature = "alloc")]
type FrameBytes = Vec<u8>;
#[cfg(not(feature = "alloc"))]
type FrameBytes = heapless::Vec<u8, { rmk_types::constants::RYNK_BUFFER_SIZE }>;

/// A topic frame. The no-alloc bound tracks the topic table exactly, so a
/// newer-minor firmware's extended topic (trailing bytes) is dropped there.
#[cfg(feature = "alloc")]
type TopicBytes = Vec<u8>;
#[cfg(not(feature = "alloc"))]
type TopicBytes = heapless::Vec<u8, { RYNK_HEADER_SIZE + rmk_types::protocol::rynk::MAX_TOPIC_PAYLOAD }>;

/// Queued topic frames before the oldest is dropped.
const TOPIC_QUEUE_CAPACITY: usize = 8;

/// Errors from Rynk host.
#[derive(Debug, Error)]
pub enum RynkHostError {
    #[error("transport disconnected")]
    Disconnected,
    #[error("io error: {0:?}")]
    Io(ErrorKind),
    /// A transport step (GATT attach, port open, …) failed, with its detail —
    /// what a picker/GUI shows when a chosen device can't be reached.
    #[cfg(feature = "alloc")]
    #[error("transport {0} failed: {1}")]
    Transport(&'static str, String),
    #[cfg(feature = "alloc")]
    #[error("device not found: {0}")]
    DeviceNotFound(String),

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
    /// The request failed to encode or exceeds the device's advertised
    /// `max_payload_size`.
    #[error("request {0:?} does not fit the device buffer (or failed to encode)")]
    Encode(Cmd),
    #[error("response decode failed for {cmd:?}: {source}")]
    Deserialize { cmd: Cmd, source: postcard::Error },
    /// `GetLayout` blob inflate or decode failed.
    #[cfg(feature = "alloc")]
    #[error("layout blob decode failed: {0}")]
    Layout(String),
    #[error("response for {cmd:?} had trailing bytes")]
    TrailingBytes { cmd: Cmd },
    #[error("response cmd mismatch: sent {sent:?}, got {got:?}")]
    CmdMismatch { sent: Cmd, got: Cmd },
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
            RynkHostError::Io(_) | RynkHostError::Transport(..) => "TransportError",
            RynkHostError::DeviceNotFound(_) => "DeviceNotFound",
            RynkHostError::Rejected(_) => "Rejected",
            RynkHostError::Unsupported(..) => "Unsupported",
            RynkHostError::VersionMismatch { .. } => "VersionMismatch",
            RynkHostError::Encode(_) => "RequestEncodeError",
            RynkHostError::Deserialize { .. } => "ResponseDecodeError",
            RynkHostError::Layout(_) => "LayoutDecodeError",
            RynkHostError::TrailingBytes { .. } => "ResponseTrailingBytes",
            RynkHostError::CmdMismatch { .. } => "ResponseCommandMismatch",
        };
        let err = js_sys::Error::new(&e.to_string());
        err.set_name(kind);
        err.into()
    }
}

/// The Rynk protocol surface: typed requests plus the topic stream, both
/// `&self` so a request branch and a topic branch run full-duplex over one
/// shared client. Moving the wire bytes is [`Driver::run`]'s job.
pub struct Client {
    /// Client → Driver: request frames awaiting the writer.
    message: Channel<CS, FrameBytes, 1>,
    /// Driver → requester: reply frames. A latest-wins slot — one request
    /// in flight means anything older is a stale reply.
    resp: Signal<CS, FrameBytes>,
    /// Driver → topic consumer. Drop-oldest on overflow; topics are
    /// best-effort by contract (a missed push is recovered via `get_*`).
    topics: Channel<CS, TopicBytes, TOPIC_QUEUE_CAPACITY>,
    /// Request SEQ, cycling through `1..=255`.
    next_seq: AtomicU8,
    /// Capability snapshot from the handshake; written by
    /// [`RynkDevice::connect`](crate::RynkDevice::connect) before sharing.
    pub(crate) capabilities: DeviceCapabilities,
}

impl Client {
    pub(crate) fn new() -> Self {
        Self {
            message: Channel::new(),
            resp: Signal::new(),
            topics: Channel::new(),
            next_seq: AtomicU8::new(1),
            capabilities: DeviceCapabilities::default(),
        }
    }

    /// Read the next topic push, decoded. Unrecognized topics are skipped.
    ///
    /// Parks until a topic arrives; if the link dies it never resolves — the
    /// surrounding `select` (or driver-task watch) cancels it.
    pub async fn next_topic(&self) -> TopicEvent {
        loop {
            let mut bytes = self.topics.receive().await;
            let Ok(msg) = RynkMessage::try_from(&mut bytes[..]) else {
                continue;
            };
            match TopicEvent::decode(msg.header().cmd, msg.payload()) {
                Some(event) => return event,
                None => log::debug!("rynk: unknown topic {:?}, skipped", msg.header().cmd),
            }
        }
    }

    /// One typed request/response round trip from the shared command table.
    pub(crate) async fn request<E: Endpoint>(&self, req: &E::Request) -> Result<E::Response, RynkHostError> {
        let cmd = E::CMD;
        let seq = self.send_request(cmd, req).await?;
        loop {
            // This wait has no disconnect signal; session supervision must cancel it when the driver exits.
            let mut bytes = self.resp.wait().await;
            let Ok(msg) = RynkMessage::try_from(&mut bytes[..]) else {
                continue;
            };
            let header = msg.header();
            if header.seq != seq {
                // Skip a stale reply from a cancelled request.
                continue;
            }
            if header.cmd != cmd {
                return Err(RynkHostError::CmdMismatch {
                    sent: cmd,
                    got: header.cmd,
                });
            }
            // Reject postcard prefixes so host/firmware type drift is not silently accepted.
            let (env, rest) = postcard::take_from_bytes::<Result<E::Response, RynkError>>(msg.payload())
                .map_err(|source| RynkHostError::Deserialize { cmd, source })?;
            if !rest.is_empty() {
                return Err(RynkHostError::TrailingBytes { cmd });
            }
            return env.map_err(RynkHostError::Rejected);
        }
    }

    /// Send one request frame without waiting for a reply — for commands whose
    /// effect prevents one (reboot, bootloader jump). `Ok` means the frame is
    /// queued for the writer; keep the driver running until it drains.
    pub(crate) async fn send_no_reply<E: Endpoint>(&self, req: &E::Request) -> Result<(), RynkHostError> {
        self.send_request(E::CMD, req).await.map(|_| ())
    }

    /// Encode one request into an owned frame and queue it for the writer,
    /// returning its SEQ (cycling `1..=255`).
    async fn send_request<Req: Serialize>(&self, cmd: Cmd, req: &Req) -> Result<u8, RynkHostError> {
        // `fetch_update` cannot fail because the closure always returns `Some`.
        let (Ok(seq) | Err(seq)) = self.next_seq.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |s| {
            Some(if s == u8::MAX { 1 } else { s + 1 })
        });
        // Encode against the device's limit so oversized requests fail before touching the link.
        let limit = RYNK_HEADER_SIZE + self.capabilities.max_payload_size as usize;
        #[cfg(feature = "alloc")]
        let mut buf: FrameBytes = vec![0; limit];
        #[cfg(not(feature = "alloc"))]
        let mut buf: FrameBytes = {
            let mut b = FrameBytes::new();
            let n = limit.min(b.capacity());
            b.resize(n, 0).map_err(|_| RynkHostError::Encode(cmd))?;
            b
        };
        let frame_len = RynkMessage::build(&mut buf, cmd, seq, req)
            .map_err(|_| RynkHostError::Encode(cmd))?
            .frame_len();
        buf.truncate(frame_len);
        self.message.send(buf).await;
        Ok(seq)
    }

    /// Negotiate the version, then fetch device capabilities.
    ///
    /// Rejects only major-version mismatches; same-major minors connect.
    pub(crate) async fn handshake(&self) -> Result<DeviceCapabilities, RynkHostError> {
        let version = self.request::<command::GetVersion>(&()).await?;
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
        self.request::<command::GetCapabilities>(&()).await
    }
}

/// The byte pump for one session: owns the transport halves and the RX
/// reassembly state. Protocol parsing stays in [`Client`]; the driver only
/// cuts the stream into frames and routes them by the CMD topic bit.
pub struct Driver<R: Read, W: Write> {
    reader: R,
    writer: W,
    rx: RxBuf,
}

impl<R: Read, W: Write> Driver<R, W> {
    pub(crate) fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            rx: RxBuf::new(),
        }
    }

    /// Pump both directions until the link dies, then return why.
    ///
    /// `&mut self` so it can be called repeatedly — reassembly state lives in
    /// the struct, so a cancelled run (select exit, wasm per-call) loses nothing.
    pub async fn run(&mut self, client: &Client) -> RynkHostError {
        let Self { reader, writer, rx } = self;

        let rx_loop = async {
            loop {
                // Commit in the same poll so cancelling `run` cannot lose received bytes.
                let n = match reader.read(rx.unfilled()).await {
                    Ok(0) => break RynkHostError::Disconnected,
                    Ok(n) => n,
                    Err(e) => break RynkHostError::Io(e.kind()),
                };
                rx.commit(n);
                while let Some(header) = rx.filled().first_chunk().map(RynkHeader::parse) {
                    let frame_len = header.frame_len();
                    // Fixed buffers cannot grow; discard the declared frame length to resynchronize.
                    #[cfg(not(feature = "alloc"))]
                    if frame_len > rx.buf.len() {
                        log::debug!("rynk: dropping {frame_len}-byte frame exceeding the RX buffer");
                        let mut remaining = frame_len - rx.filled().len();
                        rx.clear();
                        while remaining > 0 {
                            let cap = rx.buf.len().min(remaining);
                            match reader.read(&mut rx.buf[..cap]).await {
                                Ok(0) => return RynkHostError::Disconnected,
                                Ok(n) => remaining -= n,
                                Err(e) => return RynkHostError::Io(e.kind()),
                            }
                        }
                        continue;
                    }
                    if rx.filled().len() < frame_len {
                        break;
                    }
                    let frame = &rx.filled()[..frame_len];
                    if header.cmd.is_topic() {
                        #[cfg(feature = "alloc")]
                        let bytes = TopicBytes::from(frame);
                        #[cfg(not(feature = "alloc"))]
                        let Ok(bytes) = TopicBytes::try_from(frame) else {
                            log::debug!("rynk: oversized topic dropped");
                            rx.consume(frame_len);
                            continue;
                        };
                        if let Err(TrySendError::Full(bytes)) = client.topics.try_send(bytes) {
                            // Keep RX non-blocking by evicting the oldest best-effort topic.
                            let _ = client.topics.try_receive();
                            log::debug!("rynk: topic queue full, dropped oldest");
                            let _ = client.topics.try_send(bytes);
                        }
                    } else {
                        #[cfg(feature = "alloc")]
                        let bytes = FrameBytes::from(frame);
                        // Cannot fail: the oversize guard above caps frame_len at the RX
                        // buffer size, which is exactly FrameBytes' capacity.
                        #[cfg(not(feature = "alloc"))]
                        let bytes = FrameBytes::try_from(frame).unwrap();
                        // A pending reply is stale after request cancellation; keep the latest.
                        client.resp.signal(bytes);
                    }
                    rx.consume(frame_len);
                }
            }
        };

        let tx_loop = async {
            loop {
                let frame = client.message.receive().await;
                if let Err(e) = writer.write_all(&frame).await {
                    break RynkHostError::Io(e.kind());
                }
            }
        };

        match select(tx_loop, rx_loop).await {
            Either::First(e) | Either::Second(e) => e,
        }
    }
}

/// RX reassembly buffer: bytes land in the tail, whole frames are consumed
/// from the front by advancing a head cursor — compaction happens once per
/// `read` (in [`unfilled`](Self::unfilled)), not once per frame, so a batch
/// of frames in one read costs one memmove. Alloc builds grow the buffer on
/// demand; no-alloc builds fix it at the firmware's full-frame size and drain
/// larger frames by length.
struct RxBuf {
    #[cfg(feature = "alloc")]
    buf: Vec<u8>,
    #[cfg(not(feature = "alloc"))]
    buf: [u8; rmk_types::constants::RYNK_BUFFER_SIZE],
    head: usize,
    filled: usize,
}

/// Tail headroom kept available for each `read` on alloc builds.
#[cfg(feature = "alloc")]
const READ_CHUNK: usize = 4096;

impl RxBuf {
    fn new() -> Self {
        Self {
            #[cfg(feature = "alloc")]
            buf: vec![0; READ_CHUNK],
            #[cfg(not(feature = "alloc"))]
            buf: [0; rmk_types::constants::RYNK_BUFFER_SIZE],
            head: 0,
            filled: 0,
        }
    }

    fn filled(&self) -> &[u8] {
        &self.buf[self.head..self.filled]
    }

    fn unfilled(&mut self) -> &mut [u8] {
        if self.head > 0 {
            self.buf.copy_within(self.head..self.filled, 0);
            self.filled -= self.head;
            self.head = 0;
        }
        #[cfg(feature = "alloc")]
        if self.buf.len() - self.filled < READ_CHUNK {
            self.buf.resize(self.filled + READ_CHUNK, 0);
        }
        &mut self.buf[self.filled..]
    }

    fn commit(&mut self, n: usize) {
        self.filled += n;
    }

    fn consume(&mut self, n: usize) {
        self.head += n;
    }

    #[cfg(not(feature = "alloc"))]
    fn clear(&mut self) {
        self.head = 0;
        self.filled = 0;
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
