//! Rynk config over BLE GATT — a single per-connection session shared by both
//! transports: the custom 128-bit GATT [`RynkService`] (native bluest hosts) and
//! the vendor HID-over-GATT `RynkHidService` (browsers via WebHID).
//!
//! A connection is one host on one transport, so [`run_host_ble`] runs ONE
//! [`RynkService::run_session`]: the inbound [`RYNK_BLE_RX_PIPE`] (both transports
//! de-frame into it in `gatt_events_task`) is the Rx, and [`MuxBleTx`] routes each
//! reply/topic to whichever characteristic the host is using ([`ACTIVE_SOURCE`]) —
//! raw MTU-chunked on the custom char, or `[len][payload][zero-pad]` 32-byte
//! reports on the HID char. Returns on disconnect.

use core::sync::atomic::{AtomicU8, Ordering};

use embedded_io_async::{ErrorType, Write};
use heapless::Vec;
use rmk_types::protocol::rynk::{RYNK_BLE_CHUNK_SIZE, RYNK_HID_REPORT_SIZE};
use trouble_host::prelude::*;

use crate::ble::ble_server::Server;
use crate::channel::RYNK_BLE_RX_PIPE;
use crate::host::rynk::RynkService;
use crate::host::transport::HostTransportError;

/// Which BLE transport the connected host is using, so the single session routes
/// replies/topics to the right characteristic. Set by `gatt_events_task` only on
/// a real config WRITE — never on a CCCD subscribe, since the OS HOGP driver
/// auto-subscribes the HID input CCCD on bond, which would mis-bind a native
/// session's replies. Reset per connection; one host on one transport, no flap.
pub(crate) static ACTIVE_SOURCE: AtomicU8 = AtomicU8::new(SOURCE_NONE);
/// No transport established yet — drop topic pushes (no subscriber to notify).
pub(crate) const SOURCE_NONE: u8 = 0;
/// Custom 128-bit GATT `RynkService` (native bluest hosts).
pub(crate) const SOURCE_CUSTOM: u8 = 1;
/// Vendor HID-over-GATT `RynkHidService` (browsers over WebHID).
pub(crate) const SOURCE_HID: u8 = 2;

/// De-frame one fixed-size WebHID report into its rynk payload: strip the 1-byte
/// length prefix, clamping a malformed length to the available bytes. `len == 0`
/// (keep-alive) yields an empty slice. The byte [`RYNK_BLE_RX_PIPE`] the payload
/// is written to handles cross-read buffering, so no per-report state is needed.
pub(crate) fn hid_report_payload(report: &[u8]) -> &[u8] {
    if report.is_empty() {
        return &[];
    }
    let len = (report[0] as usize).min(RYNK_HID_REPORT_SIZE - 1).min(report.len() - 1);
    &report[1..1 + len]
}

/// Run one rynk session over `conn`, clearing stale RX bytes and the transport
/// selector from a prior connection first. Returns when the session ends.
pub async fn run_host_ble<'stack, 'server, P: PacketPool>(
    server: &'server Server<'_>,
    conn: &GattConnection<'stack, 'server, P>,
    service: &RynkService<'_>,
) {
    RYNK_BLE_RX_PIPE.clear();
    ACTIVE_SOURCE.store(SOURCE_NONE, Ordering::Relaxed);
    let mut rx = &RYNK_BLE_RX_PIPE;
    let mut tx = MuxBleTx {
        custom_input: server.rynk_service.input_data.clone(),
        hid_input: server.rynk_hid_service.input_data,
        conn,
    };
    service.run_session(&mut rx, &mut tx).await;
}

/// Write half: routes each reply/topic frame to the active transport's
/// characteristic — raw MTU-chunked on the custom char, or length-prefix-framed
/// 32-byte reports on the HID char.
struct MuxBleTx<'a, 'b, 'c, P: PacketPool> {
    custom_input: Characteristic<Vec<u8, RYNK_BLE_CHUNK_SIZE>>,
    hid_input: Characteristic<[u8; RYNK_HID_REPORT_SIZE]>,
    conn: &'a GattConnection<'b, 'c, P>,
}

impl<P: PacketPool> ErrorType for MuxBleTx<'_, '_, '_, P> {
    type Error = HostTransportError;
}

impl<P: PacketPool> Write for MuxBleTx<'_, '_, '_, P> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        match ACTIVE_SOURCE.load(Ordering::Relaxed) {
            SOURCE_HID => {
                // One length-prefixed, zero-padded 32-byte report per ≤31-byte
                // slice. N = 32 fits one notification at MTU ≥ 35.
                for chunk in buf.chunks(RYNK_HID_REPORT_SIZE - 1) {
                    let mut report = [0u8; RYNK_HID_REPORT_SIZE];
                    report[0] = chunk.len() as u8;
                    report[1..1 + chunk.len()].copy_from_slice(chunk);
                    if let Err(e) = self.hid_input.notify(self.conn, &report).await {
                        error!("Failed to notify Rynk HID reply: {:?}", e);
                        return Err(HostTransportError);
                    }
                }
            }
            SOURCE_CUSTOM => {
                // Raw, MTU-chunked — a notify past ATT_MTU − 3 is silently
                // truncated, not split, so a dropped tail would desync the host.
                let max_notify = (self.conn.raw().att_mtu() as usize).saturating_sub(3);
                let chunk_size = RYNK_BLE_CHUNK_SIZE.min(max_notify).max(1);
                for chunk in buf.chunks(chunk_size) {
                    let payload =
                        Vec::<u8, RYNK_BLE_CHUNK_SIZE>::from_slice(chunk).expect("chunk size <= RYNK_BLE_CHUNK_SIZE");
                    if let Err(e) = self.custom_input.notify(self.conn, &payload).await {
                        error!("Failed to notify Rynk reply: {:?}", e);
                        return Err(HostTransportError);
                    }
                }
            }
            // No transport established yet — drop (e.g. a topic emitted before
            // the host has written a request or subscribed for notifications).
            _ => {}
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

    /// Build one 32-byte report `[len][payload..][zero-pad]`.
    fn report(payload: &[u8]) -> [u8; RYNK_HID_REPORT_SIZE] {
        let mut r = [0u8; RYNK_HID_REPORT_SIZE];
        r[0] = payload.len() as u8;
        r[1..1 + payload.len()].copy_from_slice(payload);
        r
    }

    #[test]
    fn strips_length_prefix() {
        assert_eq!(hid_report_payload(&report(b"abc")), b"abc");
    }

    /// `len == 0` is a keep-alive — no payload to forward (the session's pipe read
    /// just blocks for the next byte; it never sees Ok(0)/EOF).
    #[test]
    fn keep_alive_is_empty() {
        assert_eq!(hid_report_payload(&report(b"")), b"");
    }

    /// A malformed length byte beyond the 31-byte payload capacity is clamped.
    #[test]
    fn clamps_oversized_length_byte() {
        let mut bad = [7u8; RYNK_HID_REPORT_SIZE];
        bad[0] = 200;
        assert_eq!(hid_report_payload(&bad).len(), RYNK_HID_REPORT_SIZE - 1);
    }

    /// A length larger than the slice is clamped to the available bytes.
    #[test]
    fn clamps_to_slice_len() {
        assert_eq!(hid_report_payload(&[5u8, 1, 2]), &[1u8, 2]);
    }

    /// Seam → pipe smoke test: a multi-report message driven through the PRODUCTION
    /// de-frame (`hid_report_payload`, exactly as the WebHID arm of `gatt_events_task`)
    /// into the real [`RYNK_BLE_RX_PIPE`] must read back through `&RYNK_BLE_RX_PIPE`
    /// — the `Read` the session consumes — as the original contiguous byte stream:
    /// prefixes/padding stripped, a `len == 0` keep-alive dropped at the seam.
    #[test]
    fn seam_strip_reassembles_clean_stream_for_session() {
        use crate::test_support::test_block_on as block_on;

        RYNK_BLE_RX_PIPE.clear();
        // 70-byte message → three reports (31 + 31 + 8 payload bytes).
        let msg: [u8; 70] = core::array::from_fn(|i| i as u8);

        // Mirror the seam: strip each report via the helper, write only the payload.
        let feed = |bytes: &[u8]| {
            let rep = report(bytes);
            let payload = hid_report_payload(&rep);
            if !payload.is_empty() {
                assert_eq!(RYNK_BLE_RX_PIPE.try_write(payload).unwrap(), payload.len());
            }
        };
        feed(&msg[0..31]);
        feed(b""); // keep-alive — the seam writes nothing
        feed(&msg[31..62]);
        feed(&msg[62..70]);

        // Read it back the way the session does (Rx = `&RYNK_BLE_RX_PIPE`).
        let rx = &RYNK_BLE_RX_PIPE;
        let mut got = [0u8; 70];
        let mut n = 0;
        while n < got.len() {
            n += block_on(rx.read(&mut got[n..]));
        }
        assert_eq!(got, msg);
    }
}
