//! Rynk config over BLE WebHID (`RynkHidService`), paired with the custom-GATT
//! [`crate::ble::rynk::run_host_ble`].
//!
//! A pure browser can't reach rynk's custom 128-bit GATT service on an
//! OS-bonded keyboard, but it *can* reach a vendor HID-over-GATT report via
//! WebHID (the report rides the OS's existing HID link). [`run_host_ble_hid`]
//! drives the *same* [`RynkService`] session over that report: fixed
//! [`RYNK_HID_REPORT_SIZE`]-byte reports framed `[len][payload 0..len][zero-pad]`,
//! where `len == 0` is a keep-alive. The Rx adapter strips the prefix and
//! buffers sub-report reads so the session sees a clean byte stream; the Tx
//! adapter splits each reply into ≤(N−1)-byte slices, one notify per report.

use embedded_io_async::{ErrorType, Read, Write};
use heapless::Vec;
use rmk_types::protocol::rynk::RYNK_HID_REPORT_SIZE;
use trouble_host::prelude::*;

use crate::ble::ble_server::Server;
use crate::channel::RYNK_HID_BLE_RX_CHANNEL;
use crate::host::rynk::RynkService;
use crate::host::transport::HostTransportError;

/// Run one rynk-over-WebHID session over `conn`, clearing stale RX reports from
/// a prior connection first. Shares `service` (and thus the `KeyMap`) with the
/// custom-GATT rynk session; the dispatch guard in `host::rynk` serializes the
/// two. Returns when the session ends.
pub async fn run_host_ble_hid<'stack, 'server, P: PacketPool>(
    server: &'server Server<'_>,
    conn: &GattConnection<'stack, 'server, P>,
    service: &RynkService<'_>,
) {
    RYNK_HID_BLE_RX_CHANNEL.clear();
    let mut rx = RynkHidBleRx {
        pending: Vec::new(),
        pos: 0,
    };
    let mut tx = RynkHidBleTx {
        input_data: server.rynk_hid_service.input_data,
        conn,
    };
    service.run_session(&mut rx, &mut tx).await;
}

/// Read half: reassembles the length-prefixed HID reports into the contiguous
/// rynk byte stream `run_session` reads. `pending`/`pos` buffer one report's
/// payload so the session's 5-byte-header-then-N reads are served across report
/// boundaries (a frame can span several reports).
struct RynkHidBleRx {
    pending: Vec<u8, RYNK_HID_REPORT_SIZE>,
    pos: usize,
}

impl ErrorType for RynkHidBleRx {
    type Error = HostTransportError;
}

impl Read for RynkHidBleRx {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            // Serve whatever payload is left from the last report first.
            if self.pos < self.pending.len() {
                let n = (self.pending.len() - self.pos).min(buf.len());
                buf[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            // Refill from the next report. `len == 0` is a keep-alive: loop for
            // the next report rather than returning Ok(0), which the session
            // reads as EOF and ends on.
            let report = RYNK_HID_BLE_RX_CHANNEL.receive().await;
            let len = (report[0] as usize).min(RYNK_HID_REPORT_SIZE - 1);
            if len == 0 {
                continue;
            }
            self.pending.clear();
            // len <= RYNK_HID_REPORT_SIZE - 1 < capacity, so this can't fail.
            let _ = self.pending.extend_from_slice(&report[1..1 + len]);
            self.pos = 0;
        }
    }
}

/// Write half: splits each reply frame into ≤(N−1)-byte slices, prepends the
/// 1-byte length and zero-pads to a fixed report, then notifies one report per
/// slice. N = 32 always fits one notification at MTU ≥ 35, so no MTU chunking.
struct RynkHidBleTx<'a, 'b, 'c, P: PacketPool> {
    input_data: Characteristic<[u8; RYNK_HID_REPORT_SIZE]>,
    conn: &'a GattConnection<'b, 'c, P>,
}

impl<P: PacketPool> ErrorType for RynkHidBleTx<'_, '_, '_, P> {
    type Error = HostTransportError;
}

impl<P: PacketPool> Write for RynkHidBleTx<'_, '_, '_, P> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        for chunk in buf.chunks(RYNK_HID_REPORT_SIZE - 1) {
            let mut report = [0u8; RYNK_HID_REPORT_SIZE];
            report[0] = chunk.len() as u8;
            report[1..1 + chunk.len()].copy_from_slice(chunk);
            if let Err(e) = self.input_data.notify(self.conn, &report).await {
                error!("Failed to notify Rynk HID reply: {:?}", e);
                return Err(HostTransportError);
            }
        }
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_block_on as block_on;

    /// Build one HID report `[len][payload..][zero-pad]`.
    fn report(payload: &[u8]) -> [u8; RYNK_HID_REPORT_SIZE] {
        let mut r = [0u8; RYNK_HID_REPORT_SIZE];
        r[0] = payload.len() as u8;
        r[1..1 + payload.len()].copy_from_slice(payload);
        r
    }

    fn rx() -> RynkHidBleRx {
        RynkHidBleRx {
            pending: Vec::new(),
            pos: 0,
        }
    }

    #[test]
    fn strips_length_prefix() {
        RYNK_HID_BLE_RX_CHANNEL.clear();
        assert!(RYNK_HID_BLE_RX_CHANNEL.try_send(report(b"abc")).is_ok());
        let mut rx = rx();
        let mut buf = [0u8; RYNK_HID_REPORT_SIZE];
        let n = block_on(rx.read(&mut buf)).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf[..3], b"abc");
    }

    /// `run_session` reads a 5-byte header then N payload bytes, so the Rx must
    /// serve arbitrary sub-report slices from its `pending` buffer.
    #[test]
    fn buffers_sub_report_reads() {
        RYNK_HID_BLE_RX_CHANNEL.clear();
        assert!(RYNK_HID_BLE_RX_CHANNEL.try_send(report(b"abc")).is_ok());
        let mut rx = rx();
        let mut one = [0u8; 1];
        assert_eq!(block_on(rx.read(&mut one)).unwrap(), 1);
        assert_eq!(one[0], b'a');
        assert_eq!(block_on(rx.read(&mut one)).unwrap(), 1);
        assert_eq!(one[0], b'b');
        let mut big = [0u8; 8];
        assert_eq!(block_on(rx.read(&mut big)).unwrap(), 1);
        assert_eq!(big[0], b'c');
    }

    /// A frame larger than one report arrives across several reports; the Rx
    /// hands back contiguous bytes so the session reassembles transparently.
    #[test]
    fn concatenates_successive_reports() {
        RYNK_HID_BLE_RX_CHANNEL.clear();
        let first = [0xAAu8; RYNK_HID_REPORT_SIZE - 1]; // 31 payload bytes (a full report)
        assert!(RYNK_HID_BLE_RX_CHANNEL.try_send(report(&first)).is_ok());
        assert!(RYNK_HID_BLE_RX_CHANNEL.try_send(report(b"hello")).is_ok());
        let mut rx = rx();
        let mut buf = [0u8; 64];
        assert_eq!(block_on(rx.read(&mut buf)).unwrap(), 31);
        assert_eq!(&buf[..31], &first[..]);
        assert_eq!(block_on(rx.read(&mut buf)).unwrap(), 5);
        assert_eq!(&buf[..5], b"hello");
    }

    /// `len == 0` is a keep-alive — the Rx must loop for the next report, never
    /// returning Ok(0) (which the session reads as EOF and ends on).
    #[test]
    fn keep_alive_is_not_eof() {
        RYNK_HID_BLE_RX_CHANNEL.clear();
        assert!(RYNK_HID_BLE_RX_CHANNEL.try_send(report(b"")).is_ok()); // keep-alive
        assert!(RYNK_HID_BLE_RX_CHANNEL.try_send(report(b"xy")).is_ok());
        let mut rx = rx();
        let mut buf = [0u8; 8];
        let n = block_on(rx.read(&mut buf)).unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf[..2], b"xy");
    }

    /// A malformed length byte beyond the 31-byte payload capacity is clamped,
    /// so the slice index can't go out of bounds.
    #[test]
    fn clamps_oversized_length_byte() {
        RYNK_HID_BLE_RX_CHANNEL.clear();
        let mut bad = [7u8; RYNK_HID_REPORT_SIZE];
        bad[0] = 200;
        assert!(RYNK_HID_BLE_RX_CHANNEL.try_send(bad).is_ok());
        let mut rx = rx();
        let mut buf = [0u8; 64];
        assert_eq!(block_on(rx.read(&mut buf)).unwrap(), RYNK_HID_REPORT_SIZE - 1);
    }
}
