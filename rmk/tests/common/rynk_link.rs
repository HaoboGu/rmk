//! In-memory embedded-io duplex + host-side client for end-to-end Rynk tests.
//!
//! The whole Rynk stack is transport-agnostic: `RynkService::run_session` is
//! generic over [`embedded_io_async::Read`] + [`Write`], and the USB / BLE /
//! UART adapters just hand it a byte stream. This harness hands it the same
//! kind of byte stream, built from two `embassy_sync::pipe::Pipe`s (both
//! implement the embedded-io traits), and plays the host on the other end.
//!
//! ```text
//!   RynkClient ──request──▶  h2d pipe  ──▶ run_session (device)
//!              ◀─response──  d2h pipe  ◀──
//!              ◀──topic────
//! ```
//!
//! Every exchange crosses the production framing/codec path
//! (parse → dispatch → handler → response-encode → framing) plus the
//! topic-emit and oversized-frame resync arms of `run_session` — so tests
//! exercise the entire Rynk service independent of any hardware transport.
//!
//! This harness deliberately re-implements the host half instead of pulling in
//! `rynk`'s real `Client`: as a dev-dependency it would force
//! `rmk-types/host` (= `rynk+bulk+_ble+split`) via Cargo feature unification in
//! *every* rmk test build, so a lean config (`--features rynk,storage`) would
//! compile the `_ble`/`bulk` `Cmd` variants while `run_session`'s match arms
//! stay cfg'd out — a non-exhaustive match. The host and this harness instead
//! share one codec (`rmk-types`) and decode responses with identical strictness
//! (`take_from_bytes` rejecting trailing bytes), which is what keeps the two
//! ends from drifting.

use embassy_futures::join::join;
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pipe::Pipe;
use embedded_io_async::Read;
use rmk::host::HostService as RynkService;
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{Cmd, RYNK_HEADER_SIZE, RynkError, RynkHeader, RynkMessage};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::test_block_on::test_block_on;

/// One direction of the link. Sized to a full Rynk buffer so any single legal
/// frame fits, and the writer never deadlocks waiting on a reader that has not
/// been polled yet.
pub type Link = Pipe<NoopRawMutex, RYNK_BUFFER_SIZE>;

/// A frame read off the wire, decoded only as far as its header.
///
pub struct Frame {
    pub header: RynkHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Decode the payload as a `Result<T, RynkError>` response envelope, strictly
    /// — trailing bytes rejected, just like the host.
    pub fn envelope<T: DeserializeOwned>(&self) -> Result<T, RynkError> {
        let (env, rest) = postcard::take_from_bytes::<Result<T, RynkError>>(&self.payload)
            .expect("response payload must decode as an envelope");
        assert!(rest.is_empty(), "response payload has {} trailing byte(s)", rest.len());
        env
    }

    /// Decode the payload as a bare `T` — topic frames are not enveloped.
    /// Deliberately stricter than the host's lenient topic decode: this
    /// pins the same-version encoder's exact output, trailing bytes included.
    pub fn raw<T: DeserializeOwned>(&self) -> T {
        let (value, rest) = postcard::take_from_bytes::<T>(&self.payload).expect("topic payload must decode");
        assert!(rest.is_empty(), "topic payload has {} trailing byte(s)", rest.len());
        value
    }
}

/// Host end of the link. Holds shared handles to both directions: `rx` reads
/// device→host bytes (responses and topic pushes), `tx` writes host→device
/// request frames. Encodes/decodes exactly as a real host (Vial tool, …) does.
pub struct RynkClient<'p> {
    rx: &'p Link,
    tx: &'p Link,
    buf: [u8; RYNK_BUFFER_SIZE],
}

impl<'p> RynkClient<'p> {
    /// Encode and send a request frame. `seq` correlates the response.
    pub async fn send<T: Serialize>(&mut self, cmd: Cmd, seq: u8, payload: &T) {
        let n = RynkMessage::build(&mut self.buf, cmd, seq, payload)
            .expect("build request frame")
            .frame_len();
        // `Pipe::write_all` is inherent and infallible (the in-memory link
        // never errors); it just blocks until the device drains enough room.
        self.tx.write_all(&self.buf[..n]).await;
    }

    /// Send hand-built bytes verbatim — for malformed / adversarial framing.
    pub async fn send_raw(&mut self, bytes: &[u8]) {
        self.tx.write_all(bytes).await;
    }

    /// Read exactly one frame off the wire: fixed header, then declared payload.
    pub async fn recv_frame(&mut self) -> Frame {
        let mut rx = self.rx;
        let mut bytes = [0u8; RYNK_HEADER_SIZE];
        rx.read_exact(&mut bytes).await.expect("read header");
        let header = RynkHeader::parse(&bytes);
        let mut payload = vec![0u8; header.payload_len as usize];
        if !payload.is_empty() {
            rx.read_exact(&mut payload).await.expect("read payload");
        }
        Frame { header, payload }
    }

    /// Read frames until one echoes `seq`, skipping any topic pushes that arrive
    /// in between. Responses echo the request's `seq`; topic frames use `seq 0`,
    /// so request seqs must be non-zero to stay unambiguous.
    pub async fn recv_response(&mut self, seq: u8) -> Frame {
        debug_assert_ne!(
            seq, 0,
            "use a non-zero request seq so topic frames (seq 0) don't alias it"
        );
        loop {
            let frame = self.recv_frame().await;
            if frame.header.seq == seq {
                return frame;
            }
            assert_eq!(
                frame.header.seq, 0,
                "unexpected frame (cmd={:?}, seq={}) while awaiting response seq {}",
                frame.header.cmd, frame.header.seq, seq,
            );
        }
    }

    /// One full request/response round-trip: send, await the correlated reply,
    /// assert the `cmd` echo, and decode the `Result<Resp, RynkError>` envelope.
    pub async fn request<Req: Serialize, Resp: DeserializeOwned>(
        &mut self,
        cmd: Cmd,
        seq: u8,
        req: &Req,
    ) -> Result<Resp, RynkError> {
        self.send(cmd, seq, req).await;
        let frame = self.recv_response(seq).await;
        assert_eq!(frame.header.cmd, cmd, "response must echo the request cmd");
        frame.envelope()
    }

    /// Await the next unsolicited topic push.
    pub async fn recv_topic(&mut self) -> Frame {
        let frame = self.recv_frame().await;
        assert!(
            frame.header.cmd.is_topic(),
            "expected a topic frame, got cmd={:?}",
            frame.header.cmd
        );
        assert_eq!(frame.header.seq, 0, "topic frames use seq 0");
        frame
    }
}

/// Run `script` (playing the host) against `service` over an in-memory
/// embedded-io duplex, returning the script's value.
///
/// `run_session` runs concurrently and is dropped once the script returns; the
/// pipes never signal EOF, so the session would otherwise loop forever. If the
/// session resolves first (a read/write error or EOF) that's a framing bug, not
/// a finished test, so we panic.
pub fn link_session<T>(service: &RynkService<'_>, script: impl AsyncFnOnce(&mut RynkClient<'_>) -> T) -> T {
    let h2d = Link::new();
    let d2h = Link::new();
    let mut dev_rx: &Link = &h2d;
    let mut dev_tx: &Link = &d2h;
    let mut client = RynkClient {
        rx: &d2h,
        tx: &h2d,
        buf: [0u8; RYNK_BUFFER_SIZE],
    };
    test_block_on(async {
        // Drive the session alongside a flash-channel drainer: a handler's
        // persistence writes (`FLASH_CHANNEL.send().await` under `storage`)
        // would otherwise block forever once the queue fills, since no storage
        // task runs here. Neither future resolves on its own; the session
        // resolving (EOF/error) means a framing bug.
        let device = select(
            service.run_session(&mut dev_rx, &mut dev_tx),
            rmk::channel::drain_flash_channel_for_test(),
        );
        match select(device, script(&mut client)).await {
            Either::First(_) => panic!("run_session ended before the client script finished"),
            Either::Second(value) => value,
        }
    })
}

/// Like [`link_session`] but runs TWO concurrent `run_session`s over the SAME
/// `service` — the production shape on a board that exposes both BLE-GATT and
/// BLE-HID (or BLE + USB), where each transport drives its own session against
/// one shared `RynkService`/`KeyMap` and its own `TopicSubscribers`. Exercises
/// the dispatch guard and the concurrent-subscriber path. `script` drives both
/// host ends.
pub fn link_two_sessions<T>(
    service: &RynkService<'_>,
    script: impl AsyncFnOnce(&mut RynkClient<'_>, &mut RynkClient<'_>) -> T,
) -> T {
    let (h2d_a, d2h_a, h2d_b, d2h_b) = (Link::new(), Link::new(), Link::new(), Link::new());
    let mut dev_rx_a: &Link = &h2d_a;
    let mut dev_tx_a: &Link = &d2h_a;
    let mut dev_rx_b: &Link = &h2d_b;
    let mut dev_tx_b: &Link = &d2h_b;
    let mut client_a = RynkClient {
        rx: &d2h_a,
        tx: &h2d_a,
        buf: [0u8; RYNK_BUFFER_SIZE],
    };
    let mut client_b = RynkClient {
        rx: &d2h_b,
        tx: &h2d_b,
        buf: [0u8; RYNK_BUFFER_SIZE],
    };
    test_block_on(async {
        // Both sessions run concurrently and never EOF; the pair is dropped once
        // the script returns. Either resolving first is a framing/guard bug.
        let devices = select(
            join(
                service.run_session(&mut dev_rx_a, &mut dev_tx_a),
                service.run_session(&mut dev_rx_b, &mut dev_tx_b),
            ),
            rmk::channel::drain_flash_channel_for_test(),
        );
        match select(devices, script(&mut client_a, &mut client_b)).await {
            Either::First(_) => panic!("a run_session ended before the client script finished"),
            Either::Second(value) => value,
        }
    })
}
