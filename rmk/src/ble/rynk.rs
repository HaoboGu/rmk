//! Rynk over BLE GATT.
//!
//! One public free function — [`run_host_ble`] — that owns the whole
//! per-connection lifecycle: clear the inbound byte pipe, construct the Tx
//! adapter around the GATT plumbing, and call [`RynkService::run_session`]
//! once with the pipe as the Rx half. Returns when the underlying
//! `embedded_io_async` halves error out (typically a disconnect); the
//! parent BLE task is the outer reconnect loop.

use embedded_io_async::{ErrorType, Write};
use heapless::Vec;
use rmk_types::protocol::rynk::RYNK_BLE_CHUNK_SIZE;
use trouble_host::prelude::*;

use crate::ble::ble_server::Server;
use crate::channel::RYNK_BLE_RX_PIPE;
use crate::host::rynk::RynkService;
use crate::host::transport::HostTransportError;

/// Run one rynk session over `conn`. Clears any leftover RX bytes from a
/// prior connection, constructs the Tx adapter in place, and returns when
/// the session ends.
pub async fn run_host_ble<'stack, 'server, P: PacketPool>(
    server: &'server Server<'_>,
    conn: &GattConnection<'stack, 'server, P>,
    service: &RynkService<'_>,
) {
    RYNK_BLE_RX_PIPE.clear();
    let mut rx = &RYNK_BLE_RX_PIPE;
    let mut tx = RynkBleTx {
        input_data: server.rynk_service.input_data.clone(),
        conn,
    };
    service.run_session(&mut rx, &mut tx).await;
}

/// Write half. Notifies the `input_data` characteristic in slices bounded by
/// the live ATT MTU (and the `RYNK_BLE_CHUNK_SIZE` characteristic capacity).
struct RynkBleTx<'a, 'b, 'c, P: PacketPool> {
    input_data: Characteristic<Vec<u8, RYNK_BLE_CHUNK_SIZE>>,
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
        // A notification larger than ATT_MTU − 3 is silently truncated by the
        // stack (trouble's `assemble` caps the PDU at the negotiated MTU) —
        // the lost bytes would desync the host's stream reassembler for the
        // rest of the session. Chunk by the live MTU, capped at the
        // characteristic capacity. (`max(1)` only guards `chunks(0)`; the
        // ATT-minimum MTU of 23 keeps real links well above it.)
        let max_notify = (self.conn.raw().att_mtu() as usize).saturating_sub(3);
        let chunk_size = RYNK_BLE_CHUNK_SIZE.min(max_notify).max(1);
        for chunk in buf.chunks(chunk_size) {
            // `chunk_size <= RYNK_BLE_CHUNK_SIZE` (the Vec capacity), so
            // from_slice cannot fail.
            let payload = Vec::<u8, RYNK_BLE_CHUNK_SIZE>::from_slice(chunk).expect("chunk size <= RYNK_BLE_CHUNK_SIZE");
            if let Err(e) = self.input_data.notify(self.conn, &payload).await {
                error!("Failed to notify Rynk reply: {:?}", e);
                return Err(HostTransportError);
            }
        }
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
