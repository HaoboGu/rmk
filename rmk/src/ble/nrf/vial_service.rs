use super::spec::{BleCharacteristics, BleDescriptor, BLE_HID_SERVICE_UUID};
use crate::ble::descriptor::{BleCompositeReportType, BleVialReport};
use defmt::{error, info, Format};
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

#[derive(Debug, defmt::Format)]
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
                Attribute::new(BleVialReport::desc()).security(SecurityMode::JustWorks),
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
            Attribute::new([BleCompositeReportType::Vial as u8, 1u8])
                .security(SecurityMode::JustWorks),
        )?;
        let input_vial_handle = input_vial.build();

        let mut output_vial = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 32]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().write_without_response()),
        )?;
        let output_vial_desc = output_vial.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Vial as u8, 2u8])
                .security(SecurityMode::JustWorks),
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

    pub(crate) fn send_ble_vial_report(&self, conn: &Connection, data: &[u8]) {
        gatt_server::notify_value(conn, self.input_vial, data)
            .map_err(|e| error!("send vial report error: {}", e))
            .ok();
    }
}

impl gatt_server::Service for BleVialService {
    type Event = VialServiceEvent;

    fn on_write(&self, handle: u16, data: &[u8]) -> Option<Self::Event> {
        if handle == self.input_vial_cccd {
            Some(VialServiceEvent::InputVialKeyCccdWrite)
        } else if handle == self.output_vial {
            info!("Vial output: {:?}", data);
            Some(VialServiceEvent::OutputVial)
        } else {
            None
        }
    }
}

#[derive(Debug, Format)]
pub(crate) enum VialServiceEvent {
    InputVialKeyCccdWrite,
    OutputVial,
}
