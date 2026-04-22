//! BLE HID transport for Vial.
//!
//! Wraps the custom GATT characteristics declared in
//! `crate::ble::host_gatt::vial::VialGattService`. A single `BleHidRxTx`
//! implements both `HostRx` (drains `VIAL_OUTPUT_CHANNEL`, populated by
//! the GATT write handler) and `HostTx` (notifies on the input
//! characteristic), because Vial's loop alternates between them.
//!
//! NOTE: `VIAL_OUTPUT_CHANNEL` is a single global static. Only one
//! `BleHidRxTx` may be live at a time — two instances would race each other
//! draining the same channel. Today exactly one is constructed per
//! connection in `run_ble_keyboard`; revisit when supporting multiple
//! simultaneous host connections.

use trouble_host::prelude::*;

use crate::ble::ble_server::Server;
use crate::ble::host::vial::VIAL_OUTPUT_CHANNEL;
use crate::host::{HostError, HostRx, HostTx};

const BLE_HID_FRAME: usize = 32;

pub(crate) struct BleHidRxTx<'stack, 'server, 'conn, P: PacketPool> {
    input_data: Characteristic<[u8; BLE_HID_FRAME]>,
    conn: &'conn GattConnection<'stack, 'server, P>,
}

impl<'stack, 'server, 'conn, P: PacketPool> BleHidRxTx<'stack, 'server, 'conn, P> {
    pub(crate) fn new(server: &Server, conn: &'conn GattConnection<'stack, 'server, P>) -> Self {
        Self {
            input_data: server.host_gatt.input_data,
            conn,
        }
    }
}

impl<P: PacketPool> HostRx for BleHidRxTx<'_, '_, '_, P> {
    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, HostError> {
        if buf.len() < BLE_HID_FRAME {
            return Err(HostError::BufferTooSmall);
        }
        let frame = VIAL_OUTPUT_CHANNEL.receive().await;
        buf[..BLE_HID_FRAME].copy_from_slice(&frame);
        Ok(BLE_HID_FRAME)
    }
}

impl<P: PacketPool> HostTx for BleHidRxTx<'_, '_, '_, P> {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), HostError> {
        if bytes.len() > BLE_HID_FRAME {
            return Err(HostError::FrameTooLarge);
        }
        let mut buf = [0u8; BLE_HID_FRAME];
        buf[..bytes.len()].copy_from_slice(bytes);
        self.input_data.notify(self.conn, &buf).await.map_err(|_| HostError::Io)
    }
}
