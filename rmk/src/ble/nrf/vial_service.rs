use defmt::{debug, error, Format};
use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use nrf_softdevice::{
    ble::{
        gatt_server::{
            self,
            builder::ServiceBuilder,
            characteristic::{Attribute, Metadata, Properties},
            RegisterError,
        },
        Connection, SecurityMode,
    },
    Softdevice,
};
use usbd_hid::descriptor::SerializedDescriptor;

use crate::{
    hid::{HidError, HidReaderTrait, HidWriterTrait},
    usb::descriptor::ViaReport,
};

use super::spec::{BleCharacteristics, BleDescriptor, BLE_HID_SERVICE_UUID};

static vial_output_channel: Channel<CriticalSectionRawMutex, [u8; 32], 4> = Channel::new();

#[derive(Debug, Clone, Copy)]
pub(crate) struct BleVialService {
    pub(crate) input_vial: u16,
    input_vial_cccd: u16,
    input_vial_descriptor: u16,
    pub(crate) output_vial: u16,
    output_vial_descriptor: u16,
    hid_info: u16,
    report_map: u16,
    hid_control: u16,
    protocol_mode: u16,
}

impl BleVialService {
    pub(crate) fn new(sd: &mut Softdevice) -> Result<Self, RegisterError> {
        let mut service_builder = ServiceBuilder::new(sd, BLE_HID_SERVICE_UUID)?;

        let hid_info_handle = service_builder
            .add_characteristic(
                BleCharacteristics::HidInfo.uuid(),
                Attribute::new([
                    0x1u8, 0x1u8,  // HID version: 1.1
                    0x00u8, // Country Code
                    0x03u8, // Remote wake + Normally Connectable
                ])
                .security(SecurityMode::JustWorks),
                Metadata::new(Properties::new().read()),
            )?
            .build();

        let report_map_handle = service_builder
            .add_characteristic(
                BleCharacteristics::ReportMap.uuid(),
                Attribute::new(ViaReport::desc()).security(SecurityMode::JustWorks),
                Metadata::new(Properties::new().read()),
            )?
            .build();

        let hid_control_handle = service_builder
            .add_characteristic(
                BleCharacteristics::HidControlPoint.uuid(),
                Attribute::new([0u8]).security(SecurityMode::JustWorks),
                Metadata::new(Properties::new().write_without_response()),
            )?
            .build();

        let protocol_mode_handle = service_builder
            .add_characteristic(
                BleCharacteristics::ProtocolMode.uuid(),
                Attribute::new([0x01u8]).security(SecurityMode::JustWorks),
                Metadata::new(Properties::new().read().write_without_response()),
            )?
            .build();

        // Existing Vial input and output characteristics
        let mut input_vial = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 32]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().notify()),
        )?;
        let input_vial_desc = input_vial.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([0u8, 1u8]).security(SecurityMode::JustWorks),
        )?;
        let input_vial_handle = input_vial.build();

        let mut output_vial = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 32]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().write_without_response()),
        )?;
        let output_vial_desc = output_vial.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([0u8, 2u8]).security(SecurityMode::JustWorks),
        )?;
        let output_vial_handle = output_vial.build();

        let _service_handle = service_builder.build();

        Ok(BleVialService {
            input_vial: input_vial_handle.value_handle,
            input_vial_cccd: input_vial_handle.cccd_handle,
            input_vial_descriptor: input_vial_desc.handle(),
            output_vial: output_vial_handle.value_handle,
            output_vial_descriptor: output_vial_desc.handle(),
            hid_info: hid_info_handle.value_handle,
            report_map: report_map_handle.value_handle,
            hid_control: hid_control_handle.value_handle,
            protocol_mode: protocol_mode_handle.value_handle,
        })
    }

    pub(crate) fn send_ble_vial_report(
        &self,
        conn: &Connection,
        data: &[u8],
    ) -> Result<(), HidError> {
        gatt_server::notify_value(conn, self.input_vial, data).map_err(|e| {
            error!("Send ble report error: {}", e);
            match e {
                gatt_server::NotifyValueError::Disconnected => HidError::BleDisconnected,
                gatt_server::NotifyValueError::Raw(_) => HidError::BleRawError,
            }
        })
    }
}

impl gatt_server::Service for BleVialService {
    type Event = VialServiceEvent;

    fn on_write(&self, handle: u16, data: &[u8]) -> Option<Self::Event> {
        if handle == self.input_vial_cccd {
            Some(VialServiceEvent::InputVialKeyCccdWrite)
        } else if handle == self.output_vial {
            debug!("Vial output: {:?}", data);
            let data = unsafe { *(data.as_ptr() as *const [u8; 32]) };
            // Retry at most 3 times
            for _ in 0..3 {
                if let Ok(_) = vial_output_channel.try_send(data) {
                    break;
                }
                // Wait for 20ms before sending the next report
                block_on(embassy_time::Timer::after_millis(20));
                error!("Vial output channel full");
            }
            Some(VialServiceEvent::OutputVial)
        } else {
            None
        }
    }
}

pub(crate) struct BleVialReaderWriter<'a> {
    pub(crate) service: BleVialService,
    pub(crate) conn: &'a Connection,
}

impl<'a> BleVialReaderWriter<'a> {
    pub(crate) fn new(service: BleVialService, conn: &'a Connection) -> Self {
        Self { service, conn }
    }
}

impl HidWriterTrait for BleVialReaderWriter<'_> {
    type ReportType = ViaReport;

    async fn get_report(&mut self) -> Self::ReportType {
        todo!()
    }

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        use ssmarshal::serialize;
        let mut buf: [u8; 32] = [0; 32];
        let n = serialize(&mut buf, &report).map_err(|_| HidError::ReportSerializeError)?;
        self.service.send_ble_vial_report(self.conn, &mut buf)?;
        Ok(n)
    }
}

impl HidReaderTrait for BleVialReaderWriter<'_> {
    type ReportType = ViaReport;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let v = vial_output_channel.receive().await;
        Ok(ViaReport {
            input_data: v,
            output_data: [0; 32],
        })
    }
}

#[derive(Debug, Format)]
pub(crate) enum VialServiceEvent {
    InputVialKeyCccdWrite,
    OutputVial,
}
