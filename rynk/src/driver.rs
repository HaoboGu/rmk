//! Protocol driver for Rynk: the [`Client`] request/topic surface and the
//! [`Driver`] byte pump. Sessions are created by
//! [`RynkDevice::connect`](crate::RynkDevice::connect).
//!
//! ## Frame flow
//!
//! A frame is a 3-byte header (`CMD u16 LE | SEQ u8`) plus a postcard payload,
//! COBS-encoded and `0x00`-terminated on the wire so the stream self-resyncs.
//! Frames cross between [`Client`] and [`Driver`] as plain owned bytes over
//! three channels; [`encode_frame`] builds them, [`RynkMessage`] parses them
//! back, and a [`Deframer`] cuts them out of the received byte stream:
//!
//! ```text
//! request()    encode → message ─→ Driver: write_all
//! Driver: read → reassemble → route by the CMD topic bit:
//!          topic frame → topics ─→ next_topic(): decode
//!          reply frame → SEQ-matched slot ─→ request(): decode
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
    Cmd, Deframer, DeviceCapabilities, ProtocolVersion, RYNK_HEADER_SIZE, RynkError, RynkHeader, RynkMessage,
    TopicEvent, command, encode_frame, max_wire_size,
};
use serde::Serialize;
use thiserror::Error;

type CS = CriticalSectionRawMutex;

/// One whole frame as owned bytes: a COBS-encoded request queued for the writer,
/// or a decoded logical reply handed back to the requester. The no-alloc bound is
/// the firmware's frame-buffer size, so any frame it can send or reply with fits.
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

/// Open exchanges the client tracks at once; a request beyond this parks on
/// the free-list until a slot drains. No-alloc builds hold a full frame buffer
/// per slot, so they keep the single-slot behavior instead of paying for four.
#[cfg(feature = "alloc")]
pub(crate) const MAX_IN_FLIGHT: usize = 4;
#[cfg(not(feature = "alloc"))]
pub(crate) const MAX_IN_FLIGHT: usize = 1;

/// One in-flight exchange: the SEQ it waits for (0 = free; issued SEQs cycle
/// `1..=255`) and the signal its reply is routed into.
struct Slot {
    seq: AtomicU8,
    resp: Signal<CS, FrameBytes>,
}

/// Frees a slot on drop, so a cancelled request releases its slot and the late
/// reply is dropped as unmatched instead of poisoning the next exchange.
struct SlotGuard<'a> {
    client: &'a Client,
    idx: usize,
}

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        let slot = &self.client.slots[self.idx];
        slot.seq.store(0, Ordering::Release);
        slot.resp.reset();
        // Capacity equals the slot count, so the free-list always has room.
        let _ = self.client.free.try_send(self.idx);
    }
}

/// The Rynk protocol surface: typed requests plus the topic stream, both
/// `&self` so request branches and a topic branch run full-duplex over one
/// shared client. Up to [`MAX_IN_FLIGHT`] requests run concurrently — replies
/// are routed back by SEQ, so completion order is independent of send order.
/// Moving the wire bytes is [`Driver::run`]'s job.
pub struct Client {
    /// Client → Driver: request frames awaiting the writer.
    message: Channel<CS, FrameBytes, 1>,
    /// In-flight exchanges; the driver routes each reply here by SEQ. A reply
    /// no slot waits for (a cancelled request's, a fire-and-forget echo) is
    /// dropped.
    slots: [Slot; MAX_IN_FLIGHT],
    /// Free slot indices — receiving one is acquisition, so callers beyond
    /// [`MAX_IN_FLIGHT`] park here instead of flooding the device.
    free: Channel<CS, usize, MAX_IN_FLIGHT>,
    /// Driver → topic consumer. Drop-oldest on overflow; topics are
    /// best-effort by contract (a missed push is recovered via `get_*`).
    topics: Channel<CS, TopicBytes, TOPIC_QUEUE_CAPACITY>,
    /// Request SEQ, cycling through `1..=255`.
    next_seq: AtomicU8,
    /// Signaled once when the driver exits, never reset: each woken waiter
    /// re-signals, so every current and future request resolves to
    /// `Disconnected` instead of parking on a dead session.
    dead: Signal<CS, ()>,
    /// Capability snapshot from the handshake; written by
    /// [`RynkDevice::connect`](crate::RynkDevice::connect) before sharing.
    pub(crate) capabilities: DeviceCapabilities,
}

impl Client {
    pub(crate) fn new() -> Self {
        let free = Channel::new();
        for idx in 0..MAX_IN_FLIGHT {
            let _ = free.try_send(idx);
        }
        Self {
            message: Channel::new(),
            slots: core::array::from_fn(|_| Slot {
                seq: AtomicU8::new(0),
                resp: Signal::new(),
            }),
            free,
            topics: Channel::new(),
            next_seq: AtomicU8::new(1),
            dead: Signal::new(),
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
    ///
    /// Runtime-free, so no deadline: a silent peer keeps this pending, and
    /// callers that need a bound wrap it in their runtime's timeout. Dropping
    /// the call is safe: [`SlotGuard`] frees the slot and the late reply is
    /// dropped as unmatched.
    pub(crate) async fn request<E: Endpoint>(&self, req: &E::Request) -> Result<E::Response, RynkHostError> {
        let cmd = E::CMD;
        let idx = self.free.receive().await;
        let _guard = SlotGuard { client: self, idx };
        let slot = &self.slots[idx];
        let seq = self.alloc_seq();
        // Claim the SEQ before the frame can hit the wire, so the reply always
        // finds its slot; `reset` drops anything a prior tenant left behind.
        slot.resp.reset();
        slot.seq.store(seq, Ordering::Release);
        self.send_frame(cmd, seq, req).await?;
        loop {
            let mut bytes = match select(slot.resp.wait(), self.dead.wait()).await {
                Either::First(bytes) => bytes,
                Either::Second(()) => {
                    self.dead.signal(()); // sticky: pass the wakeup on to other waiters
                    return Err(RynkHostError::Disconnected);
                }
            };
            let Ok(msg) = RynkMessage::try_from(&mut bytes[..]) else {
                continue;
            };
            let header = msg.header();
            if header.cmd != cmd {
                return Err(RynkHostError::CmdMismatch {
                    sent: cmd,
                    got: header.cmd,
                });
            }
            // Reject postcard prefixes so host/firmware type drift is not silently accepted.
            return match postcard::take_from_bytes::<Result<E::Response, RynkError>>(msg.payload()) {
                Err(source) => Err(RynkHostError::Deserialize { cmd, source }),
                Ok((_, rest)) if !rest.is_empty() => Err(RynkHostError::TrailingBytes { cmd }),
                Ok((env, _)) => env.map_err(RynkHostError::Rejected),
            };
        }
    }

    /// Send one request frame without waiting for a reply — for commands whose
    /// effect prevents one (reboot, bootloader jump). `Ok` means the frame is
    /// queued for the writer; keep the driver running until it drains. If the
    /// reset turns out to be a no-op, the firmware's reply matches no slot and
    /// is dropped.
    pub(crate) async fn send_no_reply<E: Endpoint>(&self, req: &E::Request) -> Result<(), RynkHostError> {
        self.send_frame(E::CMD, self.alloc_seq(), req).await
    }

    /// Next request SEQ, cycling `1..=255` — 0 marks a free slot, so it is
    /// never issued.
    fn alloc_seq(&self) -> u8 {
        // `fetch_update` cannot fail because the closure always returns `Some`.
        let (Ok(seq) | Err(seq)) = self.next_seq.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |s| {
            Some(if s == u8::MAX { 1 } else { s + 1 })
        });
        seq
    }

    /// Encode one request into an owned frame and queue it for the writer.
    async fn send_frame<Req: Serialize>(&self, cmd: Cmd, seq: u8, req: &Req) -> Result<(), RynkHostError> {
        // Size the buffer for the COBS-encoded form of a max-payload request, so an
        // oversized request fails to encode before touching the link.
        let limit = max_wire_size(RYNK_HEADER_SIZE + self.capabilities.max_payload_size as usize);
        #[cfg(feature = "alloc")]
        let mut buf: FrameBytes = vec![0; limit];
        #[cfg(not(feature = "alloc"))]
        let mut buf: FrameBytes = {
            let mut b = FrameBytes::new();
            let n = limit.min(b.capacity());
            b.resize(n, 0).map_err(|_| RynkHostError::Encode(cmd))?;
            b
        };
        let frame_len = encode_frame(&mut buf, RynkHeader { cmd, seq }, req).map_err(|_| RynkHostError::Encode(cmd))?;
        buf.truncate(frame_len);
        // Racing `dead` keeps a send from parking forever on a full queue
        // after the driver has already exited.
        match select(self.message.send(buf), self.dead.wait()).await {
            Either::First(()) => Ok(()),
            Either::Second(()) => {
                self.dead.signal(()); // sticky: pass the wakeup on to other waiters
                Err(RynkHostError::Disconnected)
            }
        }
    }

    /// Negotiate the version, then fetch device capabilities.
    ///
    /// Rejects only major-version mismatches; same-major minors connect.
    pub(crate) async fn handshake(&self) -> Result<DeviceCapabilities, RynkHostError> {
        // Both requests ride one round trip; the version gate still runs
        // before the capabilities are released.
        let (version, capabilities) = embassy_futures::join::join(
            self.request::<command::GetVersion>(&()),
            self.request::<command::GetCapabilities>(&()),
        )
        .await;
        let version = version?;
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
        capabilities
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
                let n = match reader.read(rx.tail()).await {
                    Ok(0) => break RynkHostError::Disconnected,
                    Ok(n) => n,
                    Err(e) => break RynkHostError::Io(e.kind()),
                };
                rx.commit(n);
                // Cut out every whole COBS frame this read completed; the Deframer
                // decodes in place and resyncs past any garbage on its own.
                while let Some(frame) = rx.next_frame() {
                    let header = RynkHeader::parse(frame[..RYNK_HEADER_SIZE].try_into().unwrap());
                    if header.cmd.is_topic() {
                        #[cfg(feature = "alloc")]
                        let bytes = TopicBytes::from(frame);
                        #[cfg(not(feature = "alloc"))]
                        let Ok(bytes) = TopicBytes::try_from(frame) else {
                            log::debug!("rynk: oversized topic dropped");
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
                        // A decoded frame is never larger than the RX buffer, so this fits.
                        #[cfg(not(feature = "alloc"))]
                        let bytes = FrameBytes::try_from(frame).unwrap();
                        // Route to the slot claiming this SEQ. No taker means a
                        // cancelled request's late reply or a fire-and-forget
                        // echo: drop it. A seq-0 frame would match a free slot,
                        // so it is dropped outright.
                        let waiter = if header.seq == 0 {
                            None
                        } else {
                            client
                                .slots
                                .iter()
                                .find(|s| s.seq.load(Ordering::Acquire) == header.seq)
                        };
                        match waiter {
                            Some(slot) => slot.resp.signal(bytes),
                            None => log::debug!("rynk: unmatched reply {:?} seq {}, dropped", header.cmd, header.seq),
                        }
                    }
                }
            }
        };

        let tx_loop = async {
            // Lead with a lone `0x00` so stale bytes in the peer's RX (an OS port probe,
            // a prior session's half-frame) terminate as junk instead of merging with our first request.
            if let Err(e) = writer.write_all(&[0]).await {
                return RynkHostError::Io(e.kind());
            }
            loop {
                let frame = client.message.receive().await;
                if let Err(e) = writer.write_all(&frame).await {
                    break RynkHostError::Io(e.kind());
                }
            }
        };

        let err = match select(tx_loop, rx_loop).await {
            Either::First(e) | Either::Second(e) => e,
        };
        // A cancelled `run` skips this (cancel is not death); an exited one
        // wakes every parked request with `Disconnected`.
        client.dead.signal(());
        err
    }
}

/// RX reassembly buffer: bytes land in the tail, an embedded [`Deframer`] cuts
/// whole COBS frames back out in place. Alloc builds grow on demand up to
/// [`MAX_RX_ALLOC`]; no-alloc builds fix the firmware's frame-buffer size.
struct RxBuf {
    #[cfg(feature = "alloc")]
    buf: Vec<u8>,
    #[cfg(not(feature = "alloc"))]
    buf: [u8; rmk_types::constants::RYNK_BUFFER_SIZE],
    df: Deframer,
}

/// Tail headroom kept available for each `read` on alloc builds.
#[cfg(feature = "alloc")]
const READ_CHUNK: usize = 4096;

/// Upper bound on the alloc RX buffer, so a delimiter-less stream resyncs
/// (Deframer overflow) instead of growing without bound.
#[cfg(feature = "alloc")]
const MAX_RX_ALLOC: usize = 128 * 1024;

impl RxBuf {
    fn new() -> Self {
        Self {
            #[cfg(feature = "alloc")]
            buf: vec![0; READ_CHUNK],
            #[cfg(not(feature = "alloc"))]
            buf: [0; rmk_types::constants::RYNK_BUFFER_SIZE],
            df: Deframer::new(),
        }
    }

    /// The free tail to read the next chunk into; alloc builds keep `READ_CHUNK`
    /// of headroom.
    fn tail(&mut self) -> &mut [u8] {
        #[cfg(feature = "alloc")]
        {
            let target = (self.df.filled() + READ_CHUNK).min(MAX_RX_ALLOC);
            if self.buf.len() < target {
                self.buf.resize(target, 0);
            }
        }
        self.df.tail(&mut self.buf)
    }

    fn commit(&mut self, n: usize) {
        self.df.commit(n);
        // Grow before the Deframer sees a full buffer and mistakes a still-growing
        // frame for overflow; a full buffer is real overflow only at MAX_RX_ALLOC.
        #[cfg(feature = "alloc")]
        if self.df.filled() == self.buf.len() && self.buf.len() < MAX_RX_ALLOC {
            self.buf.resize((self.buf.len() + READ_CHUNK).min(MAX_RX_ALLOC), 0);
        }
    }

    /// The next decoded frame, or `None` for "read more".
    fn next_frame(&mut self) -> Option<&[u8]> {
        let len = self.df.next(&mut self.buf)?;
        Some(&self.buf[..len])
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
