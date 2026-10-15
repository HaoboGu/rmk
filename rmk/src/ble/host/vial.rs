//! Vial over BLE GATT.
//!
//! [`HostGattHandler::run`] owns the per-connection lifecycle: clear the
//! connection's inbound chunk channel, construct 32-byte HID-report Rx/Tx adapters around
//! the GATT plumbing, and call [`VialService::run_session`] once.

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embedded_io_async::{ErrorType, Read, Write};
use trouble_host::prelude::*;

use super::HostWriteOutcome;
use crate::VIAL_CHANNEL_SIZE;
use crate::ble::ble_server::Server;
use crate::host::transport::HostTransportError;
use crate::host::via::VialService;

/// Vial packets written by this connection's host, in arrival order.
pub(crate) type VialRx = Channel<NoopRawMutex, [u8; 32], VIAL_CHANNEL_SIZE>;

/// Per-connection GATT write dispatcher for Vial over HID.
pub(crate) struct HostGattHandler<'r> {
    output_handle: u16,
    input_cccd_handle: u16,
    control_point_handle: u16,
    rx: &'r VialRx,
}

impl<'r> HostGattHandler<'r> {
    pub(crate) fn new(server: &Server<'_>, rx: &'r VialRx) -> Self {
        Self {
            output_handle: server.vial_service.output_data.handle,
            input_cccd_handle: server
                .vial_service
                .input_data
                .cccd_handle
                .expect("No CCCD for Vial input"),
            control_point_handle: server.vial_service.hid_control_point.handle,
            rx,
        }
    }

    pub(crate) async fn handle_write(&mut self, handle: u16, data: &[u8], _encrypted: bool) -> HostWriteOutcome {
        if handle == self.output_handle {
            if data.len() == 32 {
                debug!("Got Vial packet: {:?}", data);
                let mut message = [0u8; 32];
                message.copy_from_slice(data);
                self.rx.send(message).await;
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
        rx: &VialRx,
    ) {
        rx.clear();
        let mut rx = VialBleRx(rx);
        let mut tx = VialBleTx {
            input_data: server.vial_service.input_data,
            conn,
        };
        service.run_session(&mut rx, &mut tx).await;
    }
}

struct VialBleRx<'r>(&'r VialRx);

impl ErrorType for VialBleRx<'_> {
    type Error = HostTransportError;
}

impl Read for VialBleRx<'_> {
    /// Vial chunks are always 32 bytes. Callers drive this via
    /// `read_exact(&mut [u8; 32])`; smaller buffers are rejected.
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let chunk = self.0.receive().await;
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
        let rx = VialRx::new();
        let mut handler = HostGattHandler::new(&server, &rx);

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

    #[test]
    fn connections_receive_only_their_own_packets() {
        use crate::test_support::test_block_on as block_on;

        let server = Server::new_default("rmk").unwrap();
        let first = VialRx::new();
        let second = VialRx::new();
        let mut first_handler = HostGattHandler::new(&server, &first);
        let mut second_handler = HostGattHandler::new(&server, &second);
        let handle = server.vial_service.output_data.handle;

        block_on(async {
            first_handler.handle_write(handle, &[1; 32], true).await;
            second_handler.handle_write(handle, &[2; 32], true).await;
            first_handler.handle_write(handle, &[3; 32], true).await;

            let mut packet = [0; 32];
            VialBleRx(&second).read_exact(&mut packet).await.unwrap();
            assert_eq!(packet, [2; 32]);
            for expected in [1, 3] {
                VialBleRx(&first).read_exact(&mut packet).await.unwrap();
                assert_eq!(packet, [expected; 32]);
            }
        });
        assert!(first.is_empty());
        assert!(second.is_empty());
    }
}
