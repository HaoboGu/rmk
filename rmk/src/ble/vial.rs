#![cfg(feature = "vial")]
use ssmarshal::serialize;
use trouble_host::prelude::*;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::ble::Server;
use crate::channel::VIAL_READ_CHANNEL;
use crate::descriptor::ViaReport;
use crate::hid::{HidError, HidReaderTrait, HidWriterTrait};

#[gatt_service(uuid = service::HUMAN_INTERFACE_DEVICE)]
pub(crate) struct ViaService {
    #[characteristic(uuid = "2a4a", read, value = [0x01, 0x01, 0x00, 0x03])]
    pub(crate) hid_info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = ViaReport::desc().try_into().expect("Failed to convert ViaReport to [u8; 27]"))]
    pub(crate) report_map: [u8; 27],
    #[characteristic(uuid = "2a4c", write_without_response)]
    pub(crate) hid_control_point: u8,
    #[characteristic(uuid = "2a4e", read, write_without_response, value = 1)]
    pub(crate) protocol_mode: u8,
    #[descriptor(uuid = "2908", read, value = [0u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify)]
    pub(crate) input_via: [u8; 32],
    #[descriptor(uuid = "2908", read, value = [0u8, 2u8])]
    #[characteristic(uuid = "2a4d", read, write, write_without_response)]
    pub(crate) output_via: [u8; 32],
}

pub(crate) struct BleViaServer<'stack, 'server, 'conn, P: PacketPool> {
    pub(crate) input_via: Characteristic<[u8; 32]>,
    pub(crate) output_via: Characteristic<[u8; 32]>,
    pub(crate) conn: &'conn GattConnection<'stack, 'server, P>,
}

impl<'stack, 'server, 'conn, P: PacketPool> BleViaServer<'stack, 'server, 'conn, P> {
    pub(crate) fn new(server: &Server, conn: &'conn GattConnection<'stack, 'server, P>) -> Self {
        Self {
            input_via: server.via_service.input_via,
            output_via: server.via_service.output_via,
            conn,
        }
    }
}

impl<P: PacketPool> HidWriterTrait for BleViaServer<'_, '_, '_, P> {
    type ReportType = ViaReport;

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        let mut buf = [0u8; 32];
        let n = serialize(&mut buf, &report).map_err(|_| HidError::ReportSerializeError)?;
        debug!("Sending via report: {:?}", buf);
        self.input_via.notify(self.conn, &buf).await.map_err(|e| {
            error!("Failed to notify via report: {:?}", e);
            HidError::BleError
        })?;
        Ok(n)
    }
}

impl<P: PacketPool> HidReaderTrait for BleViaServer<'_, '_, '_, P> {
    type ReportType = ViaReport;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let v = VIAL_READ_CHANNEL.receive().await;
        Ok(ViaReport {
            input_data: [0u8; 32],
            output_data: v,
        })
    }
}
