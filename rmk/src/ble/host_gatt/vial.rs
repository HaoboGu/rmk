use embassy_sync::channel::Channel;
use ssmarshal::serialize;
use trouble_host::prelude::*;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::ble::Server;
use crate::descriptor::ViaReport;
use crate::hid::{HidError, HidReaderTrait, HidWriterTrait};
#[cfg(feature = "split")]
use crate::split::ble::central::CENTRAL_SLEEP;

static HOST_GUI_INPUT_CHANNEL: Channel<crate::RawMutex, [u8; 32], { crate::VIAL_CHANNEL_SIZE }> = Channel::new();

#[gatt_service(uuid = service::HUMAN_INTERFACE_DEVICE)]
pub(crate) struct VialGattService {
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
    pub(crate) input_data: [u8; 32],
    #[descriptor(uuid = "2908", read, value = [0u8, 2u8])]
    #[characteristic(uuid = "2a4d", read, write, write_without_response)]
    pub(crate) output_data: [u8; 32],
}

pub(crate) enum HostGattWriteResult {
    Handled,
    CccdUpdated,
    NotHandled,
}

pub(crate) struct BleVialTransport<'stack, 'server, 'conn, P: PacketPool> {
    pub(crate) input_data: Characteristic<[u8; 32]>,
    pub(crate) conn: &'conn GattConnection<'stack, 'server, P>,
}

impl<'stack, 'server, 'conn, P: PacketPool> BleVialTransport<'stack, 'server, 'conn, P> {
    pub(crate) fn new(server: &Server, conn: &'conn GattConnection<'stack, 'server, P>) -> Self {
        Self {
            input_data: server.host_gatt.input_data,
            conn,
        }
    }
}

impl<P: PacketPool> HidWriterTrait for BleVialTransport<'_, '_, '_, P> {
    type ReportType = ViaReport;

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        let mut buf = [0u8; 32];
        let n = serialize(&mut buf, &report).map_err(|_| HidError::ReportSerializeError)?;
        debug!("Sending via report: {:?}", buf);
        self.input_data.notify(self.conn, &buf).await.map_err(|e| {
            error!("Failed to notify via report: {:?}", e);
            HidError::BleError
        })?;
        Ok(n)
    }
}

impl<P: PacketPool> HidReaderTrait for BleVialTransport<'_, '_, '_, P> {
    type ReportType = ViaReport;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let v = HOST_GUI_INPUT_CHANNEL.receive().await;
        Ok(ViaReport {
            input_data: [0u8; 32],
            output_data: v,
        })
    }
}

pub(crate) async fn handle_gatt_write(server: &Server<'_>, handle: u16, data: &[u8]) -> HostGattWriteResult {
    if handle == server.host_gatt.output_data.handle {
        debug!("Got host packet: {:?}", data);
        if data.len() == 32 {
            let mut packet = [0u8; 32];
            packet.copy_from_slice(data);
            HOST_GUI_INPUT_CHANNEL.send(packet).await;
        } else {
            warn!("Wrong host packet data: {:?}", data);
        }
        HostGattWriteResult::Handled
    } else if handle == server.host_gatt.input_data.cccd_handle.expect("No CCCD for input host") {
        HostGattWriteResult::CccdUpdated
    } else if handle == server.host_gatt.hid_control_point.handle {
        info!("Write GATT Event to Control Point: {:?}", handle);
        #[cfg(feature = "split")]
        if data.len() == 1 {
            let state = data[0];
            if state == 0 {
                CENTRAL_SLEEP.signal(true);
            } else if state == 1 {
                CENTRAL_SLEEP.signal(false);
            }
        }
        HostGattWriteResult::Handled
    } else {
        HostGattWriteResult::NotHandled
    }
}
