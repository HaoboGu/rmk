//! HID-framed variant of [`super::rynk_link`]: interposes the fixed 32-byte HID
//! report framing (firmware `RynkHidService`, fed whole into `RYNK_BLE_RX_PIPE`
//! by the `ble::rynk` WebHID arm and reply-framed by `RynkBleTx`) between the
//! host client and `run_session`, so the framing round-trips through the *real*
//! dispatcher.
//!
//! The two pipes carry whole 32-byte reports, each a fragment of the COBS frame
//! stream (final report zero-padded); the `0x00` delimiter bounds each frame and
//! the padding drops out as empty frames in the receiver's `Deframer`. The
//! device-side `HidRx`/`HidTx` and the client mirror the firmware framing;
//! `run_session` itself sees the raw report byte stream and is unchanged.

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pipe::Pipe;
use embedded_io_async::{ErrorType, Read, Write};
use rmk::host::HostService as RynkService;
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{Cmd, Deframer, RYNK_HID_REPORT_SIZE, RynkHeader, encode_frame};
use serde::Serialize;

use super::rynk_link::{Frame, RynkHostClient};
use super::test_block_on::test_block_on;

/// One direction of the link, carrying whole HID reports.
pub type Link = Pipe<NoopRawMutex, RYNK_BUFFER_SIZE>;

/// Host reassembly buffer: a full COBS frame plus one report of trailing padding.
const HID_RXBUF: usize = RYNK_BUFFER_SIZE + RYNK_HID_REPORT_SIZE;

/// Fragment `data` (one frame) into fixed 32-byte reports, the final one
/// zero-padded, and write each to `link`. Mirrors the firmware HID framing.
async fn write_framed(link: &Link, data: &[u8]) {
    for chunk in data.chunks(RYNK_HID_REPORT_SIZE) {
        let mut report = [0u8; RYNK_HID_REPORT_SIZE];
        report[..chunk.len()].copy_from_slice(chunk);
        link.write_all(&report).await;
    }
}

/// Device-side Rx: hands whole reports (padding included) to `run_session`,
/// exactly as the firmware's WebHID arm feeds `RYNK_BLE_RX_PIPE`;
/// `pending`/`pos` stand in for the pipe's byte buffering.
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
            self.pending.clear();
            self.pending.extend_from_slice(&report);
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
    df: Deframer,
    rxbuf: [u8; HID_RXBUF],
}

impl RynkHostClient for RynkHidClient<'_> {
    /// Encode a request frame and write it as fixed 32-byte report fragments.
    async fn send<T: Serialize>(&mut self, cmd: Cmd, seq: u8, payload: &T) {
        let n = encode_frame(&mut self.buf, RynkHeader { cmd, seq }, payload).expect("build request frame");
        write_framed(self.tx, &self.buf[..n]).await;
    }

    /// Read whole reports until the Deframer yields one rynk frame; the reports'
    /// zero-padding is skipped as empty frames.
    async fn recv_frame(&mut self) -> Frame {
        loop {
            if let Some(frame) = Frame::next_from(&mut self.df, &mut self.rxbuf) {
                return frame;
            }
            let mut link: &Link = self.rx;
            let mut report = [0u8; RYNK_HID_REPORT_SIZE];
            link.read_exact(&mut report).await.expect("read report");
            let tail = self.df.tail(&mut self.rxbuf);
            tail[..RYNK_HID_REPORT_SIZE].copy_from_slice(&report);
            self.df.commit(RYNK_HID_REPORT_SIZE);
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
    };
    let mut dev_tx = HidTx { link: &d2h };
    let mut client = RynkHidClient {
        rx: &d2h,
        tx: &h2d,
        buf: [0u8; RYNK_BUFFER_SIZE],
        df: Deframer::new(),
        rxbuf: [0u8; HID_RXBUF],
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
