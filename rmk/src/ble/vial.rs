//! Vial over BLE GATT (paired with [`crate::ble::rynk::run_host_ble`]).
//!
//! One public free function — [`run_host_ble`] — owns the whole
//! per-connection lifecycle: clear the inbound chunk channel, construct
//! 32-byte HID-report Rx/Tx adapters around the GATT plumbing, and call
//! [`VialService::run_session`] once.

use embedded_io_async::{ErrorType, Read, Write};
use trouble_host::prelude::*;

use crate::ble::ble_server::Server;
use crate::channel::VIAL_BLE_RX_CHANNEL;
use crate::host::transport::HostTransportError;
use crate::host::via::VialService;

/// Run one Vial session over `conn`. Clears leftover RX chunks from a
/// prior connection and returns when the session ends.
pub async fn run_host_ble<'stack, 'server, P: PacketPool>(
    server: &'server Server<'_>,
    conn: &GattConnection<'stack, 'server, P>,
    service: &VialService<'_>,
) {
    VIAL_BLE_RX_CHANNEL.clear();
    let mut rx = VialBleRx;
    let mut tx = VialBleTx {
        input_data: server.vial_service.input_data,
        conn,
    };
    service.run_session(&mut rx, &mut tx).await;
}

struct VialBleRx;

impl ErrorType for VialBleRx {
    type Error = HostTransportError;
}

impl Read for VialBleRx {
    /// Vial chunks are always 32 bytes. Callers drive this via
    /// `read_exact(&mut [u8; 32])`; smaller buffers are rejected.
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let chunk = VIAL_BLE_RX_CHANNEL.receive().await;
        if buf.len() < chunk.len() {
            error!(
                "VialBleRx::read called with buf.len() = {} < chunk.len() = {}",
                buf.len(),
                chunk.len()
            );
            return Err(HostTransportError);
        }
        buf[..chunk.len()].copy_from_slice(&chunk);
        Ok(chunk.len())
    }
}

struct VialBleTx<'a, 'b, 'c, P: PacketPool> {
    input_data: Characteristic<[u8; 32]>,
    conn: &'a GattConnection<'b, 'c, P>,
}

impl<P: PacketPool> ErrorType for VialBleTx<'_, '_, '_, P> {
    type Error = HostTransportError;
}

impl<P: PacketPool> Write for VialBleTx<'_, '_, '_, P> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        // The GATT input_data characteristic is fixed-size [u8; 32]; expect
        // exactly 32 bytes per write call.
        let arr: &[u8; 32] = buf.try_into().map_err(|_| {
            error!("Vial reply must be exactly 32 bytes, got {}", buf.len());
            HostTransportError
        })?;
        if let Err(e) = self.input_data.notify(self.conn, arr).await {
            error!("Failed to notify Vial reply: {:?}", e);
            return Err(HostTransportError);
        }
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
