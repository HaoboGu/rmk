//! Vial over BLE GATT.
//!
//! [`HostGattHandler::run`] owns the per-connection lifecycle: clear the
//! inbound chunk channel, construct 32-byte HID-report Rx/Tx adapters around
//! the GATT plumbing, and call [`VialService::run_session`] once.

use embedded_io_async::{ErrorType, Read, Write};
use trouble_host::prelude::*;

use super::HostWriteOutcome;
use crate::ble::ble_server::Server;
use crate::channel::VIAL_BLE_RX_CHANNEL;
use crate::host::transport::HostTransportError;
use crate::host::via::VialService;

/// Per-connection GATT write dispatcher for Vial over HID.
pub(crate) struct HostGattHandler {
    output_handle: u16,
    input_cccd_handle: u16,
    control_point_handle: u16,
}

impl HostGattHandler {
    pub(crate) fn new(server: &Server<'_>) -> Self {
        Self {
            output_handle: server.vial_service.output_data.handle,
            input_cccd_handle: server
                .vial_service
                .input_data
                .cccd_handle
                .expect("No CCCD for Vial input"),
            control_point_handle: server.vial_service.hid_control_point.handle,
        }
    }

    pub(crate) async fn handle_write(&mut self, handle: u16, data: &[u8], _encrypted: bool) -> HostWriteOutcome {
        if handle == self.output_handle {
            if data.len() == 32 {
                debug!("Got Vial packet: {:?}", data);
                let mut message = [0u8; 32];
                message.copy_from_slice(data);
                VIAL_BLE_RX_CHANNEL.send(message).await;
            } else {
                warn!("Wrong Vial packet data: {:?}", data);
            }
            HostWriteOutcome::Handled
        } else if handle == self.input_cccd_handle {
            HostWriteOutcome::CccdUpdated
        } else if handle == self.control_point_handle {
            HostWriteOutcome::ControlPoint
        } else {
            HostWriteOutcome::Unhandled
        }
    }

    /// Run one Vial session over `conn`, clearing stale RX chunks first.
    pub(crate) async fn run<'stack, 'server, P: PacketPool>(
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
        if let Err(e) = self.input_data.notify(self.conn, arr, true).await {
            error!("Failed to notify Vial reply: {:?}", e);
            return Err(HostTransportError);
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
    fn classifies_vial_gatt_handles() {
        use crate::test_support::test_block_on as block_on;

        let server = Server::new_default("rmk").unwrap();
        let mut handler = HostGattHandler::new(&server);

        assert_eq!(
            block_on(handler.handle_write(server.vial_service.output_data.handle, &[], true)),
            HostWriteOutcome::Handled
        );
        assert_eq!(
            block_on(handler.handle_write(server.vial_service.input_data.cccd_handle.unwrap(), &[], true,)),
            HostWriteOutcome::CccdUpdated
        );
        assert_eq!(
            block_on(handler.handle_write(server.vial_service.hid_control_point.handle, &[0], true,)),
            HostWriteOutcome::ControlPoint
        );
        assert_eq!(
            block_on(handler.handle_write(u16::MAX, &[], true)),
            HostWriteOutcome::Unhandled
        );
    }
}
