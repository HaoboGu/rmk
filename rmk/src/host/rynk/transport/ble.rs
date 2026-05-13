//! BLE GATT transport for the Rynk service.
//!
//! Mirrors the USB transport's responsibilities for a GATT pipe:
//!
//! * Reassemble inbound frames from the chunk stream that
//!   [`gatt_events_task`](crate::ble::gatt_events_task) pushes onto
//!   [`RYNK_RX_CHANNEL`](crate::channel::RYNK_RX_CHANNEL).
//! * Dispatch each frame via [`RynkService::dispatch`].
//! * Send replies as `input_data` notifications, chunked to fit within the
//!   currently negotiated MTU (the characteristic's value type is a
//!   variable-length `heapless::Vec`, so each notify carries only its actual
//!   payload bytes).
//!
//! Unlike the USB transport, BLE does not have its own `wait_enabled` flow
//! and the [`GattConnection`] lives outside this future; the runner is
//! therefore invoked per-connection (see `run_ble_rynk` below) from
//! `run_ble_keyboard`, the same way `run_ble_host` is invoked for the Vial
//! pipe.

use embassy_futures::select::{Either, select};
use heapless::Vec;
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{RYNK_HEADER_SIZE, RynkMessage};
use trouble_host::prelude::*;

use super::super::topics::TopicSubscribers;
use super::super::{RYNK_BLE_CHUNK_SIZE, RynkService};
use crate::ble::ble_server::Server;
use crate::channel::{BLE_RYNK_READY, RYNK_RX_CHANNEL};

/// Variable-length value type of the Rynk `input_data` / `output_data`
/// characteristics. Re-exposed locally so callers don't have to depend on
/// `heapless` directly.
type RynkChunk = Vec<u8, RYNK_BLE_CHUNK_SIZE>;

/// Per-connection BLE Rynk runner. Drains [`RYNK_RX_CHANNEL`], dispatches each
/// fully reassembled frame via [`RynkService::dispatch`], and writes replies
/// back as `input_data` notifications chunked to MTU − 3.
///
/// Mirrors `crate::host::ble::run_ble_host` for the Rynk pipe. Returns when
/// the GATT connection drops (any notify failure breaks the loop). Designed
/// to be joined with the other per-connection futures in `run_ble_keyboard`
/// via `select!` so cancellation cleans up state for the next session.
pub(crate) async fn run_ble_rynk<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    service: &RynkService<'_>,
) {
    // `Characteristic<heapless::Vec<u8, N>>` derives `Copy` with an implicit
    // `T: Copy` bound that `heapless::Vec` doesn't satisfy, so clone the
    // handle wrapper. The wrapper holds attribute handles, not the value.
    let input_data = server.rynk_service.input_data.clone();

    // Discard any chunks left in the channel from a previous session so
    // we never reassemble across connections.
    RYNK_RX_CHANNEL.clear();
    BLE_RYNK_READY.reset();

    let mut rx_buf = [0u8; RYNK_BUFFER_SIZE];
    let mut rx_used: usize = 0;
    let mut dispatch_buf = [0u8; RYNK_BUFFER_SIZE];
    let mut topics = TopicSubscribers::new();

    loop {
        match select(RYNK_RX_CHANNEL.receive(), topics.next_event(&service.ctx)).await {
            Either::First(chunk) => {
                if append_chunk(&mut rx_buf, &mut rx_used, &chunk).is_err() {
                    // Frame longer than the configured buffer — either the host is
                    // misbehaving or `RYNK_BUFFER_SIZE` was hand-shrunk below
                    // `RYNK_MIN_BUFFER_SIZE` (the compile-time assert in
                    // `host/rynk/mod.rs` should make this impossible). Drop the
                    // in-progress state and resync on the next host write.
                    warn!("Rynk RX overflow; resyncing");
                    rx_used = 0;
                    continue;
                }

                // A single chunk may carry multiple back-to-back frames or only
                // part of one — drain whatever full frames are present.
                while let Some(frame_len) = parse_frame_len(&rx_buf[..rx_used]) {
                    // Copy the request into the dispatch buffer; the handler
                    // rewrites the payload in place over the full buffer
                    // capacity and patches LEN.
                    dispatch_buf[..frame_len].copy_from_slice(&rx_buf[..frame_len]);
                    // dispatch writes the response envelope (Ok or Err)
                    // into the buffer; no further error handling needed.
                    service.dispatch(&mut dispatch_buf).await;
                    match dispatch_buf.payload_len() {
                        Ok(n) => {
                            let resp_len = RYNK_HEADER_SIZE + n as usize;
                            if notify_frame(&input_data, conn, &dispatch_buf[..resp_len])
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(e) => {
                            // Unreachable in practice: dispatch_buf is sized at
                            // RYNK_BUFFER_SIZE >= RYNK_HEADER_SIZE.
                            warn!("Rynk dispatch_buf header read failed: {:?}", e);
                        }
                    }
                    rx_buf.copy_within(frame_len..rx_used, 0);
                    rx_used -= frame_len;
                }
            }
            Either::Second(event) => {
                match event
                    .encode(service, &mut dispatch_buf)
                    .and_then(|()| dispatch_buf.payload_len())
                {
                    Ok(n) => {
                        let resp_len = RYNK_HEADER_SIZE + n as usize;
                        if notify_frame(&input_data, conn, &dispatch_buf[..resp_len])
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(e) => {
                        // Unreachable in practice: dispatch_buf is sized at
                        // RYNK_BUFFER_SIZE >= RYNK_HEADER_SIZE.
                        warn!("Rynk topic encode failed: {:?}", e);
                    }
                }
            }
        }
    }
}

/// Append `chunk` to the receive buffer. Returns `Err` when the chunk
/// would push the total past `rx_buf.len()` — caller's responsibility to
/// resync.
fn append_chunk(rx_buf: &mut [u8], rx_used: &mut usize, chunk: &[u8]) -> Result<(), ()> {
    let end = rx_used.checked_add(chunk.len()).ok_or(())?;
    if end > rx_buf.len() {
        return Err(());
    }
    rx_buf[*rx_used..end].copy_from_slice(chunk);
    *rx_used = end;
    Ok(())
}

/// If the buffer holds at least one complete frame, return its byte
/// length (`RYNK_HEADER_SIZE + LEN`). Otherwise return `None`.
fn parse_frame_len(buf: &[u8]) -> Option<usize> {
    if buf.len() < RYNK_HEADER_SIZE {
        return None;
    }
    let len = u16::from_le_bytes([buf[3], buf[4]]) as usize;
    let total = RYNK_HEADER_SIZE + len;
    (buf.len() >= total).then_some(total)
}

/// Send a fully assembled frame as one or more notifications. Each notify
/// is up to `RYNK_BLE_CHUNK_SIZE` bytes — the GATT macro caps the
/// characteristic's `Vec` capacity at that value.
async fn notify_frame<P: PacketPool>(
    input_data: &Characteristic<RynkChunk>,
    conn: &GattConnection<'_, '_, P>,
    frame: &[u8],
) -> Result<(), Error> {
    for chunk in frame.chunks(RYNK_BLE_CHUNK_SIZE) {
        let payload = RynkChunk::from_slice(chunk).map_err(|_| Error::OutOfMemory)?;
        if let Err(e) = input_data.notify(conn, &payload).await {
            error!("Failed to notify Rynk reply: {:?}", e);
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    //! Reassembly logic — the only piece testable without a live BLE stack.

    extern crate std;

    use super::*;

    fn frame(payload: &[u8]) -> std::vec::Vec<u8> {
        let len = payload.len() as u16;
        let mut v = std::vec![0xCD, 0xAB, 0x42, len as u8, (len >> 8) as u8];
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn parse_frame_len_needs_full_header() {
        assert_eq!(parse_frame_len(&[]), None);
        assert_eq!(parse_frame_len(&[0; 4]), None);
    }

    #[test]
    fn parse_frame_len_returns_total_size() {
        let f = frame(&[1, 2, 3]);
        assert_eq!(parse_frame_len(&f), Some(f.len()));
    }

    #[test]
    fn parse_frame_len_short_payload_returns_none() {
        let mut f = frame(&[1, 2, 3, 4]);
        f.pop(); // drop one payload byte
        assert_eq!(parse_frame_len(&f), None);
    }

    #[test]
    fn append_chunk_concatenates_then_overflows() {
        let mut rx = [0u8; 8];
        let mut used = 0;
        append_chunk(&mut rx, &mut used, &[1, 2, 3]).unwrap();
        append_chunk(&mut rx, &mut used, &[4, 5]).unwrap();
        assert_eq!(&rx[..used], &[1, 2, 3, 4, 5]);

        // 5 + 4 > 8 → overflow.
        assert!(append_chunk(&mut rx, &mut used, &[6, 7, 8, 9]).is_err());
        // Used unchanged on failure.
        assert_eq!(used, 5);
    }
}
