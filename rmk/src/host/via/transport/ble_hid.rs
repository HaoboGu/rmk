//! BLE HID transport for Vial.
//!
//! Wraps the custom GATT characteristics declared in
//! `crate::ble::host::vial::VialGattService`. Implements `HidReaderTrait` /
//! `HidWriterTrait` with `ReportType = ViaReport` — same pattern as the
//! keyboard's `BleHidServer`.
//!
//! NOTE: `VIAL_OUTPUT_CHANNEL` is a single global static. Only one
//! `BleVialReaderWriter` may be live at a time — two instances would race
//! each other draining the same channel. Today exactly one is constructed
//! per connection in `run_ble_keyboard`; revisit when supporting multiple
//! simultaneous host connections.

use trouble_host::prelude::*;
use usbd_hid::descriptor::AsInputReport as _;

use crate::ble::ble_server::Server;
use crate::ble::host::vial::VIAL_OUTPUT_CHANNEL;
use crate::descriptor::ViaReport;
use crate::hid::{HidError, HidReaderTrait, HidWriterTrait};

const BLE_HID_FRAME: usize = 32;

pub(crate) struct BleVialReaderWriter<'stack, 'server, 'conn, P: PacketPool> {
    input_data: Characteristic<[u8; BLE_HID_FRAME]>,
    conn: &'conn GattConnection<'stack, 'server, P>,
}

impl<'stack, 'server, 'conn, P: PacketPool> BleVialReaderWriter<'stack, 'server, 'conn, P> {
    pub(crate) fn new(server: &Server, conn: &'conn GattConnection<'stack, 'server, P>) -> Self {
        Self {
            input_data: server.host_gatt.input_data,
            conn,
        }
    }
}

impl<P: PacketPool> HidReaderTrait for BleVialReaderWriter<'_, '_, '_, P> {
    type ReportType = ViaReport;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let v = VIAL_OUTPUT_CHANNEL.receive().await;
        Ok(ViaReport {
            input_data: [0u8; BLE_HID_FRAME],
            output_data: v,
        })
    }
}

impl<P: PacketPool> HidWriterTrait for BleVialReaderWriter<'_, '_, '_, P> {
    type ReportType = ViaReport;

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        let mut buf = [0u8; BLE_HID_FRAME];
        let n = report
            .serialize(&mut buf)
            .map_err(|_| HidError::ReportSerializeError)?;
        self.input_data.notify(self.conn, &buf).await.map_err(|e| {
            error!("Failed to notify via report: {:?}", e);
            HidError::BleError
        })?;
        Ok(n)
    }
}
