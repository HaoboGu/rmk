//! HID-framed variant of [`super::rynk_link`]: interposes the fixed 32-byte HID
//! report framing (firmware `RynkHidService`, de-framed at the `ble::rynk` seam
//! via `RynkHidFrameTracker` and reply-framed by `RynkBleTx`) between the host
//! client and `run_session`, so the framing round-trips through the *real*
//! dispatcher.
//!
//! The two pipes carry whole 32-byte reports, each a fragment of the rynk frame
//! stream (final report zero-padded); the frame header's LEN delimits the frame.
//! The device-side `HidRx`/`HidTx` and the client mirror the firmware framing;
//! `run_session` itself sees a clean contiguous byte stream and is unchanged.

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pipe::Pipe;
use embedded_io_async::{ErrorType, Read, Write};
use rmk::host::HostService as RynkService;
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{Cmd, RYNK_HEADER_SIZE, RYNK_HID_REPORT_SIZE, RynkHeader, RynkMessage};
use serde::Serialize;

use super::rynk_link::{Frame, RynkHostClient};
use super::test_block_on::test_block_on;

/// One direction of the link, carrying whole HID reports.
pub type Link = Pipe<NoopRawMutex, RYNK_BUFFER_SIZE>;

/// Fragment `data` (one frame) into fixed 32-byte reports, the final one
/// zero-padded, and write each to `link`. Mirrors the firmware HID framing.
async fn write_framed(link: &Link, data: &[u8]) {
    for chunk in data.chunks(RYNK_HID_REPORT_SIZE) {
        let mut report = [0u8; RYNK_HID_REPORT_SIZE];
        report[..chunk.len()].copy_from_slice(chunk);
        link.write_all(&report).await;
    }
}

/// Device-side Rx: reads whole reports off the pipe and de-frames them into the
/// byte stream `run_session` reads. Mirrors the firmware de-frame (`ble::rynk`'s
/// `RynkHidFrameTracker` feeding `RYNK_BLE_RX_PIPE`), with `pending`/`pos` standing
/// in for the pipe's buffering and `remaining` tracking the in-flight frame.
struct HidRx<'p> {
    link: &'p Link,
    pending: Vec<u8>,
    pos: usize,
    remaining: usize,
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
            if self.remaining == 0 {
                self.remaining = RYNK_HEADER_SIZE + u16::from_le_bytes([report[3], report[4]]) as usize;
            }
            let take = self.remaining.min(RYNK_HID_REPORT_SIZE);
            self.remaining -= take;
            self.pending.clear();
            self.pending.extend_from_slice(&report[..take]);
            self.pos = 0;
        }
    }
}

/// Device-side Tx: frames `run_session`'s whole-frame writes into reports onto
/// the pipe. Mirrors the firmware reply framing (`ble::rynk::RynkBleTx`, HID arm).
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

impl RynkHostClient for RynkHidClient<'_> {
    /// Encode a request frame and write it as fixed 32-byte report fragments.
    async fn send<T: Serialize>(&mut self, cmd: Cmd, seq: u8, payload: &T) {
        let msg = RynkMessage::build(&mut self.buf, cmd, seq, payload).expect("build request frame");
        write_framed(self.tx, msg.frame()).await;
    }

    /// Read whole reports and reassemble exactly one rynk frame — reports carry a
    /// fragment of the frame, so this may consume several; the header LEN delimits
    /// it and the final report's padding is dropped.
    async fn recv_frame(&mut self) -> Frame {
        let mut link: &Link = self.rx;
        let mut stream: Vec<u8> = Vec::new();
        let mut remaining = 0usize;
        loop {
            let mut report = [0u8; RYNK_HID_REPORT_SIZE];
            link.read_exact(&mut report).await.expect("read report");
            if remaining == 0 {
                remaining = RYNK_HEADER_SIZE + u16::from_le_bytes([report[3], report[4]]) as usize;
            }
            let take = remaining.min(RYNK_HID_REPORT_SIZE);
            remaining -= take;
            stream.extend_from_slice(&report[..take]);
            if remaining == 0 {
                let mut head = [0u8; RYNK_HEADER_SIZE];
                head.copy_from_slice(&stream[..RYNK_HEADER_SIZE]);
                let header = RynkHeader::parse(&head);
                let payload = stream[RYNK_HEADER_SIZE..].to_vec();
                return Frame { header, payload };
            }
        }
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
        remaining: 0,
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
