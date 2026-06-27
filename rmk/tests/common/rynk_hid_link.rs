//! HID-framed variant of [`super::rynk_link`]: interposes the 32-byte
//! length-prefix HID report framing (firmware `RynkHidService`, de-framed at the
//! `ble::rynk` seam via `hid_report_payload` and reply-framed by `MuxBleTx`)
//! between the host client and `run_session`, so the framing round-trips through
//! the *real* dispatcher.
//!
//! The two pipes carry whole HID reports `[len][payload 0..len][zero-pad to 32]`
//! (`len == 0` = keep-alive). The device-side `HidRx`/`HidTx` mirror the firmware's
//! HID framing (`hid_report_payload` de-frame + `MuxBleTx` reply framing); the
//! client frames/de-frames symmetrically. `run_session` itself sees a clean
//! contiguous byte stream and is unchanged — exactly as the production seam intends.

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pipe::Pipe;
use embedded_io_async::{ErrorType, Read, Write};
use rmk::host::HostService as RynkService;
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{Cmd, RYNK_HEADER_SIZE, RYNK_HID_REPORT_SIZE, RynkError, RynkHeader, RynkMessage};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::test_block_on::test_block_on;

/// One direction of the link, carrying whole HID reports.
pub type Link = Pipe<NoopRawMutex, RYNK_BUFFER_SIZE>;

/// Usable payload bytes per report (the rest is the 1-byte length prefix).
const PAYLOAD: usize = RYNK_HID_REPORT_SIZE - 1;

/// Frame `data` into length-prefixed, zero-padded reports and write each whole
/// report to `link`. Mirrors the firmware HID reply framing (`ble::rynk::MuxBleTx`).
async fn write_framed(link: &Link, data: &[u8]) {
    for chunk in data.chunks(PAYLOAD) {
        let mut report = [0u8; RYNK_HID_REPORT_SIZE];
        report[0] = chunk.len() as u8;
        report[1..1 + chunk.len()].copy_from_slice(chunk);
        link.write_all(&report).await;
    }
}

/// A frame read off the wire, decoded only as far as its header.
pub struct Frame {
    pub header: RynkHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Decode the payload as a `Result<T, RynkError>` envelope, strictly.
    pub fn envelope<T: DeserializeOwned>(&self) -> Result<T, RynkError> {
        let (env, rest) = postcard::take_from_bytes::<Result<T, RynkError>>(&self.payload)
            .expect("response payload must decode as an envelope");
        assert!(rest.is_empty(), "response payload has {} trailing byte(s)", rest.len());
        env
    }

    /// Decode the payload as a bare `T` — topic frames are not enveloped.
    pub fn raw<T: DeserializeOwned>(&self) -> T {
        let (value, rest) = postcard::take_from_bytes::<T>(&self.payload).expect("topic payload must decode");
        assert!(rest.is_empty(), "topic payload has {} trailing byte(s)", rest.len());
        value
    }
}

/// Device-side Rx: reads whole reports off the pipe and de-frames them into the
/// byte stream `run_session` reads. Mirrors the firmware de-frame — `ble::rynk`'s
/// seam strip (`hid_report_payload`) feeding `RYNK_BLE_RX_PIPE` — with `pending`/
/// `pos` standing in for the pipe's buffering; a `len == 0` keep-alive loops
/// rather than signalling EOF.
struct HidRx<'p> {
    link: &'p Link,
    pending: Vec<u8>,
    pos: usize,
}

impl ErrorType for HidRx<'_> {
    type Error = core::convert::Infallible;
}

impl Read for HidRx<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            if self.pos < self.pending.len() {
                let n = (self.pending.len() - self.pos).min(buf.len());
                buf[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            let mut link: &Link = self.link;
            let mut report = [0u8; RYNK_HID_REPORT_SIZE];
            link.read_exact(&mut report).await.expect("read report");
            let len = (report[0] as usize).min(PAYLOAD);
            if len == 0 {
                continue;
            }
            self.pending.clear();
            self.pending.extend_from_slice(&report[1..1 + len]);
            self.pos = 0;
        }
    }
}

/// Device-side Tx: frames `run_session`'s whole-frame writes into reports onto
/// the pipe. Mirrors the firmware reply framing (`ble::rynk::MuxBleTx`, HID arm).
struct HidTx<'p> {
    link: &'p Link,
}

impl ErrorType for HidTx<'_> {
    type Error = core::convert::Infallible;
}

impl Write for HidTx<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        write_framed(self.link, buf).await;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Host end of the link. Frames requests into reports and reassembles responses
/// from reports, sharing the `rmk-types` codec with the device.
pub struct RynkHidClient<'p> {
    rx: &'p Link,
    tx: &'p Link,
    buf: [u8; RYNK_BUFFER_SIZE],
}

impl RynkHidClient<'_> {
    /// Encode a request frame and write it as length-prefixed reports.
    pub async fn send<T: Serialize>(&mut self, cmd: Cmd, seq: u8, payload: &T) {
        let n = RynkMessage::build(&mut self.buf, cmd, seq, payload)
            .expect("build request frame")
            .frame_len();
        write_framed(self.tx, &self.buf[..n]).await;
    }

    /// Read whole reports, de-frame, and reassemble exactly one rynk frame —
    /// reports can carry a fraction of a frame, so this may consume several.
    pub async fn recv_frame(&mut self) -> Frame {
        let mut link: &Link = self.rx;
        let mut stream: Vec<u8> = Vec::new();
        loop {
            if stream.len() >= RYNK_HEADER_SIZE {
                let mut head = [0u8; RYNK_HEADER_SIZE];
                head.copy_from_slice(&stream[..RYNK_HEADER_SIZE]);
                let header = RynkHeader::parse(&head);
                let frame_len = RYNK_HEADER_SIZE + header.payload_len as usize;
                if stream.len() >= frame_len {
                    let payload = stream[RYNK_HEADER_SIZE..frame_len].to_vec();
                    return Frame { header, payload };
                }
            }
            let mut report = [0u8; RYNK_HID_REPORT_SIZE];
            link.read_exact(&mut report).await.expect("read report");
            let len = (report[0] as usize).min(PAYLOAD);
            stream.extend_from_slice(&report[1..1 + len]);
        }
    }

    /// Read frames until one echoes `seq`, skipping any topic pushes (seq 0).
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

    /// One full request/response round-trip across the HID framing.
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

/// Run `script` (playing the host) against `service` with HID report framing
/// interposed on both ends; returns the script's value. Same lifecycle contract
/// as [`super::rynk_link::link_session`]: the session resolving first is a
/// framing bug, so we panic.
pub fn link_session_hid<T>(service: &RynkService<'_>, script: impl AsyncFnOnce(&mut RynkHidClient<'_>) -> T) -> T {
    let h2d = Link::new();
    let d2h = Link::new();
    let mut dev_rx = HidRx {
        link: &h2d,
        pending: Vec::new(),
        pos: 0,
    };
    let mut dev_tx = HidTx { link: &d2h };
    let mut client = RynkHidClient {
        rx: &d2h,
        tx: &h2d,
        buf: [0u8; RYNK_BUFFER_SIZE],
    };
    test_block_on(async {
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
