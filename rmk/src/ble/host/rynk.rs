//! Rynk config over BLE GATT — a single per-connection session shared by both
//! transports: the custom 128-bit `RynkGattService` (native bluest hosts) and
//! the vendor HID-over-GATT `RynkHidService` (browsers via WebHID).
//!
//! A connection is one host on one transport, so [`HostGattHandler::run`] runs ONE
//! [`RynkService::run_session`]: the inbound [`RYNK_BLE_RX_PIPE`] (both transports
//! de-frame into it in `gatt_events_task`) is the Rx, and [`RynkBleTx`] routes each
//! reply/topic to whichever characteristic the host is using ([`RynkBleSource`]).
//! Both carry the same rynk frames, just fragmented to the transport packet —
//! MTU-chunked on the custom char, fixed 32-byte reports on the HID char. Returns
//! on disconnect.

use core::sync::atomic::{AtomicU8, Ordering};

use embedded_io_async::{ErrorType, Write};
use heapless::Vec;
#[cfg(test)]
use rmk_types::protocol::rynk::RYNK_HEADER_SIZE;
use rmk_types::protocol::rynk::{RYNK_BLE_CHUNK_SIZE, RYNK_HID_REPORT_SIZE, RynkHeader};
use trouble_host::prelude::*;

use super::HostWriteOutcome;
use crate::ble::ble_server::Server;
use crate::channel::RYNK_BLE_RX_PIPE;
use crate::host::rynk::RynkService;
use crate::host::transport::HostTransportError;

/// Per-connection GATT write dispatcher for the two Rynk BLE transports.
pub(crate) struct HostGattHandler {
    custom_output_handle: u16,
    custom_input_cccd_handle: u16,
    hid_output_handle: u16,
    hid_input_cccd_handle: u16,
    hid_control_point_handle: u16,
    hid_frame_tracker: RynkHidFrameTracker,
}

impl HostGattHandler {
    pub(crate) fn new(server: &Server<'_>) -> Self {
        Self {
            custom_output_handle: server.rynk_service.output_data.handle,
            custom_input_cccd_handle: server
                .rynk_service
                .input_data
                .cccd_handle
                .expect("No CCCD for Rynk input"),
            hid_output_handle: server.rynk_hid_service.output_data.handle,
            hid_input_cccd_handle: server
                .rynk_hid_service
                .input_data
                .cccd_handle
                .expect("No CCCD for Rynk HID input"),
            hid_control_point_handle: server.rynk_hid_service.hid_control_point.handle,
            hid_frame_tracker: RynkHidFrameTracker::new(),
        }
    }

    pub(crate) async fn handle_write(&mut self, handle: u16, data: &[u8], encrypted: bool) -> HostWriteOutcome {
        if handle == self.custom_output_handle {
            if !data.is_empty() {
                if encrypted {
                    debug!("Got Rynk packet ({} bytes)", data.len());
                    // Await the pipe's backpressure when the consumer falls behind.
                    RYNK_BLE_RX_PIPE.write_all(data).await;
                    RynkBleSource::Custom.activate();
                } else {
                    warn!("Rynk: dropping {}-byte write on unencrypted link", data.len());
                }
            }
            HostWriteOutcome::Handled
        } else if handle == self.custom_input_cccd_handle {
            // A subscription alone must not bind the reply transport. OS HOGP
            // drivers may subscribe automatically when restoring a bond.
            HostWriteOutcome::CccdUpdated
        } else if handle == self.hid_output_handle {
            if encrypted {
                if data.len() == RYNK_HID_REPORT_SIZE {
                    let bytes = self.hid_frame_tracker.take(data);
                    RYNK_BLE_RX_PIPE.write_all(bytes).await;
                    RynkBleSource::Hid.activate();
                } else {
                    warn!("Wrong Rynk HID report size: {}", data.len());
                }
            } else {
                warn!("Rynk HID: dropping {}-byte write on unencrypted link", data.len());
            }
            HostWriteOutcome::Handled
        } else if handle == self.hid_input_cccd_handle {
            HostWriteOutcome::CccdUpdated
        } else if handle == self.hid_control_point_handle {
            HostWriteOutcome::ControlPoint
        } else {
            HostWriteOutcome::Unhandled
        }
    }

    /// Run one Rynk session over `conn`, clearing stale RX bytes and the
    /// transport selector from a prior connection first.
    pub(crate) async fn run<'stack, 'server, P: PacketPool>(
        server: &'server Server<'_>,
        conn: &GattConnection<'stack, 'server, P>,
        service: &RynkService<'_>,
    ) {
        RYNK_BLE_RX_PIPE.clear();
        RynkBleSource::None.activate();
        let mut rx = &RYNK_BLE_RX_PIPE;
        let mut tx = RynkBleTx {
            custom_input: server.rynk_service.input_data.clone(),
            hid_input: server.rynk_hid_service.input_data,
            conn,
        };
        service.run_session(&mut rx, &mut tx).await;
    }
}

/// Which BLE transport the host is using, so the session routes replies/topics to
/// the right characteristic. Set only on a config WRITE, never on a CCCD subscribe
/// — the OS HOGP driver auto-subscribes the HID input CCCD on bond, which would
/// mis-bind a native session's replies. Reset per connection.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum RynkBleSource {
    /// No transport yet — drop topic pushes (no subscriber).
    None,
    /// Custom 128-bit `RynkGattService` (native hosts).
    Custom,
    /// Vendor HID-over-GATT `RynkHidService` (browsers over WebHID).
    Hid,
}

static ACTIVE_SOURCE: AtomicU8 = AtomicU8::new(RynkBleSource::None as u8);

impl RynkBleSource {
    pub(crate) fn activate(self) {
        ACTIVE_SOURCE.store(self as u8, Ordering::Relaxed);
    }

    fn active() -> Self {
        match ACTIVE_SOURCE.load(Ordering::Relaxed) {
            v if v == Self::Custom as u8 => Self::Custom,
            v if v == Self::Hid as u8 => Self::Hid,
            _ => Self::None,
        }
    }
}

/// Drops fixed 32-byte WebHID reports' zero-padding to recover the contiguous
/// rynk byte stream. `remaining` tracks the in-flight frame (0 at a boundary) so
/// only its final, padded report gets trimmed.
pub(crate) struct RynkHidFrameTracker {
    remaining: usize,
}

impl RynkHidFrameTracker {
    pub(crate) const fn new() -> Self {
        Self { remaining: 0 }
    }

    /// One report's real frame bytes, padding dropped. At a frame boundary the
    /// LEN comes from the report header; mid-frame reports pass through whole.
    pub(crate) fn take<'r>(&mut self, report: &'r [u8]) -> &'r [u8] {
        if self.remaining == 0 {
            let Some(frame_len) = RynkHeader::peek_frame_len(report) else {
                return &[];
            };
            self.remaining = frame_len;
        }
        let n = self.remaining.min(report.len());
        self.remaining -= n;
        &report[..n]
    }
}

/// Write half: routes each reply/topic frame to the active transport's
/// characteristic — MTU-chunked on the custom char, or fixed 32-byte report
/// fragments on the HID char.
struct RynkBleTx<'a, 'b, 'c, P: PacketPool> {
    custom_input: Characteristic<Vec<u8, RYNK_BLE_CHUNK_SIZE>>,
    hid_input: Characteristic<[u8; RYNK_HID_REPORT_SIZE]>,
    conn: &'a GattConnection<'b, 'c, P>,
}

impl<P: PacketPool> ErrorType for RynkBleTx<'_, '_, '_, P> {
    type Error = HostTransportError;
}

impl<P: PacketPool> Write for RynkBleTx<'_, '_, '_, P> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        match RynkBleSource::active() {
            RynkBleSource::Hid => {
                // Fragment into fixed 32-byte reports; LEN strips final padding.
                for chunk in buf.chunks(RYNK_HID_REPORT_SIZE) {
                    let mut report = [0u8; RYNK_HID_REPORT_SIZE];
                    report[..chunk.len()].copy_from_slice(chunk);
                    if let Err(e) = self.hid_input.notify(self.conn, &report, true).await {
                        error!("Failed to notify Rynk HID reply: {:?}", e);
                        return Err(HostTransportError);
                    }
                }
            }
            RynkBleSource::Custom => {
                // Raw, MTU-chunked — a notify past ATT_MTU − 3 is silently
                // truncated, not split, so a dropped tail would desync the host.
                let max_notify = (self.conn.raw().att_mtu() as usize).saturating_sub(3);
                let chunk_size = RYNK_BLE_CHUNK_SIZE.min(max_notify).max(1);
                for chunk in buf.chunks(chunk_size) {
                    let payload =
                        Vec::<u8, RYNK_BLE_CHUNK_SIZE>::from_slice(chunk).expect("chunk size <= RYNK_BLE_CHUNK_SIZE");
                    if let Err(e) = self.custom_input.notify(self.conn, &payload, true).await {
                        error!("Failed to notify Rynk reply: {:?}", e);
                        return Err(HostTransportError);
                    }
                }
            }
            // No transport established yet — drop (e.g. a topic emitted before
            // the host has written a request or subscribed for notifications).
            RynkBleSource::None => {}
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

    #[test]
    fn classifies_rynk_gatt_handles() {
        use crate::test_support::test_block_on as block_on;

        let server = Server::new_default("rmk").unwrap();
        let mut handler = HostGattHandler::new(&server);

        assert_eq!(
            block_on(handler.handle_write(server.rynk_service.output_data.handle, &[], true)),
            HostWriteOutcome::Handled
        );
        assert_eq!(
            block_on(handler.handle_write(server.rynk_hid_service.output_data.handle, &[], true,)),
            HostWriteOutcome::Handled
        );
        assert_eq!(
            block_on(handler.handle_write(server.rynk_service.input_data.cccd_handle.unwrap(), &[], true,)),
            HostWriteOutcome::CccdUpdated
        );
        assert_eq!(
            block_on(handler.handle_write(server.rynk_hid_service.input_data.cccd_handle.unwrap(), &[], true,)),
            HostWriteOutcome::CccdUpdated
        );
        assert_eq!(
            block_on(handler.handle_write(server.rynk_hid_service.hid_control_point.handle, &[0], true,)),
            HostWriteOutcome::ControlPoint
        );
        assert_eq!(
            block_on(handler.handle_write(u16::MAX, &[], true)),
            HostWriteOutcome::Unhandled
        );
    }

    /// A 70-byte frame (header LEN = 65) fragmented into 32-byte reports and
    /// de-fragmented via the header LEN reassembles exactly, padding dropped.
    fn frame_70() -> [u8; 70] {
        let mut frame = [0u8; 70];
        frame[3..5].copy_from_slice(&65u16.to_le_bytes());
        for (i, b) in frame.iter_mut().enumerate().skip(RYNK_HEADER_SIZE) {
            *b = i as u8;
        }
        frame
    }

    /// Seam → pipe: fragment a frame, de-frame each report through the PRODUCTION
    /// `RynkHidFrameTracker` (exactly as the WebHID arm of `gatt_events_task`), feed the real bytes
    /// to [`RYNK_BLE_RX_PIPE`], and read it back through `&RYNK_BLE_RX_PIPE` — the
    /// `Read` the session consumes — as the original contiguous frame.
    #[test]
    fn fragments_reassemble_through_pipe_for_session() {
        use crate::test_support::test_block_on as block_on;

        RYNK_BLE_RX_PIPE.clear();
        let frame = frame_70();

        let mut tracker = RynkHidFrameTracker::new();
        for chunk in frame.chunks(RYNK_HID_REPORT_SIZE) {
            let mut report = [0u8; RYNK_HID_REPORT_SIZE];
            report[..chunk.len()].copy_from_slice(chunk);
            let bytes = tracker.take(&report);
            assert_eq!(RYNK_BLE_RX_PIPE.try_write(bytes).unwrap(), bytes.len());
        }
        assert_eq!(tracker.remaining, 0);

        let rx = &RYNK_BLE_RX_PIPE;
        let mut got = [0u8; 70];
        let mut n = 0;
        while n < got.len() {
            n += block_on(rx.read(&mut got[n..]));
        }
        assert_eq!(got, frame);
    }

    /// A frame smaller than one report: only header + payload are taken, the rest
    /// of the report is padding.
    #[test]
    fn small_frame_drops_padding() {
        let mut report = [0u8; RYNK_HID_REPORT_SIZE];
        report[3..5].copy_from_slice(&2u16.to_le_bytes()); // LEN = 2 → 7-byte frame
        report[5..7].copy_from_slice(&[0xAA, 0xBB]);
        let mut tracker = RynkHidFrameTracker::new();
        assert_eq!(tracker.take(&report), &report[..7]);
        assert_eq!(tracker.remaining, 0);
    }
}
