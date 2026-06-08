//! Rynk over BLE GATT.
//!
//! [`run_host_ble`] runs one per-connection session: the inbound pipe as Rx,
//! a notify-based Tx adapter as the write half. Returns on disconnect.

use embedded_io_async::{ErrorType, Write};
use heapless::Vec;
use rmk_types::protocol::rynk::RYNK_BLE_CHUNK_SIZE;
use trouble_host::prelude::*;

use crate::ble::ble_server::Server;
use crate::channel::RYNK_BLE_RX_PIPE;
use crate::host::rynk::RynkService;
use crate::host::transport::HostTransportError;

/// Run one rynk session over `conn`, clearing stale RX bytes from a prior
/// connection first. Returns when the session ends.
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

/// Write half: notifies `input_data` in MTU-bounded chunks.
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
        // A notification past ATT_MTU − 3 is silently truncated, not split, so
        // chunk to fit — a dropped tail would desync the host's stream.
        let max_notify = (self.conn.raw().att_mtu() as usize).saturating_sub(3);
        let chunk_size = RYNK_BLE_CHUNK_SIZE.min(max_notify).max(1);
        for chunk in buf.chunks(chunk_size) {
            // chunk_size <= RYNK_BLE_CHUNK_SIZE, so from_slice can't fail.
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
