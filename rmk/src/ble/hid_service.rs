use defmt::{error, info};
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
use usbd_hid::descriptor::SerializedDescriptor as _;

use super::{
    descriptor::{BleCompositeReportType, BleKeyboardReport},
    spec::{BleCharacteristics, BleDescriptor, BLE_HID_SERVICE_UUID},
};

#[allow(dead_code)]
pub struct HidService {
    hid_info: u16,
    report_map: u16,
    hid_control: u16,
    protocol_mode: u16,
    pub(crate) input_keyboard: u16,
    input_keyboard_cccd: u16,
    input_keyboard_descriptor: u16,
    pub(crate) output_keyboard: u16,
    output_keyboard_descriptor: u16,
    pub(crate) input_media_keys: u16,
    input_media_keys_cccd: u16,
    input_media_keys_descriptor: u16,
    pub(crate) input_mouse_keys: u16,
    input_mouse_keys_cccd: u16,
    input_mouse_keys_descriptor: u16,
    pub(crate) input_system_keys: u16,
    input_system_keys_cccd: u16,
    input_system_keys_descriptor: u16,
    pub(crate) input_vial_keys: u16,
    input_vial_keys_cccd: u16,
    input_vial_keys_descriptor: u16,
    pub(crate) output_vial: u16,
    output_vial_descriptor: u16,
}

impl HidService {
    pub fn new(sd: &mut Softdevice) -> Result<Self, RegisterError> {
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
                Attribute::new(BleKeyboardReport::desc()).security(SecurityMode::JustWorks),
                Metadata::new(Properties::new().read()),
            )?
            .build();

        let hid_control_handle = service_builder
            .add_characteristic(
                BleCharacteristics::HidControlPoint.uuid(),
                Attribute::new([0u8]).security(SecurityMode::JustWorks),
                Metadata::new(Properties::new().read().write_without_response()),
            )?
            .build();

        let protocol_mode_handle = service_builder
            .add_characteristic(
                BleCharacteristics::ProtocolMode.uuid(),
                Attribute::new([1u8]).security(SecurityMode::JustWorks),
                Metadata::new(Properties::new().read().write_without_response()),
            )?
            .build();

        let mut input_keyboard = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 8]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().notify()),
        )?;
        let input_keyboard_desc = input_keyboard.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Keyboard as u8, 1u8]) // First is report ID, second is in/out
                .security(SecurityMode::JustWorks),
        )?;
        let input_keyboard_handle = input_keyboard.build();

        let mut output_keyboard = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 8]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().write_without_response()),
        )?;
        let output_keyboard_desc = output_keyboard.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Keyboard as u8, 2u8])
                .security(SecurityMode::JustWorks),
        )?;
        let output_keyboard_handle = output_keyboard.build();

        let mut input_media_keys = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 2]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().notify()),
        )?;
        let input_media_keys_desc = input_media_keys.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Media as u8, 1u8])
                .security(SecurityMode::JustWorks),
        )?;
        let input_media_keys_handle = input_media_keys.build();

        let mut input_system_keys = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 1]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().notify()),
        )?;
        let input_system_keys_desc = input_system_keys.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::System as u8, 1u8])
                .security(SecurityMode::JustWorks),
        )?;
        let input_system_keys_handle = input_system_keys.build();

        let mut input_mouse = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 5]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().notify()),
        )?;
        let input_mouse_desc = input_mouse.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Mouse as u8, 1u8])
                .security(SecurityMode::JustWorks),
        )?;
        let input_mouse_handle = input_mouse.build();

        let mut input_vial = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 32]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().notify()),
        )?;
        let input_vial_desc = input_vial.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Vial as u8, 1u8])
                .security(SecurityMode::JustWorks),
        )?;
        let input_vial_handle = input_vial.build();

        let mut output_vial = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 8]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().write().write_without_response()),
        )?;
        let output_vial_desc = output_vial.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Vial as u8, 2u8]) // First is report ID, second is in/out
                .security(SecurityMode::JustWorks),
        )?;
        let output_vial_handle = output_vial.build();

        let _service_handle = service_builder.build();

        Ok(HidService {
            hid_info: hid_info_handle.value_handle,
            report_map: report_map_handle.value_handle,
            hid_control: hid_control_handle.value_handle,
            protocol_mode: protocol_mode_handle.value_handle,
            input_keyboard: input_keyboard_handle.value_handle,
            input_keyboard_cccd: input_keyboard_handle.cccd_handle,
            input_keyboard_descriptor: input_keyboard_desc.handle(),
            output_keyboard: output_keyboard_handle.value_handle,
            output_keyboard_descriptor: output_keyboard_desc.handle(),
            input_media_keys: input_media_keys_handle.value_handle,
            input_media_keys_cccd: input_media_keys_handle.cccd_handle,
            input_media_keys_descriptor: input_media_keys_desc.handle(),
            input_system_keys: input_system_keys_handle.value_handle,
            input_system_keys_cccd: input_system_keys_handle.cccd_handle,
            input_system_keys_descriptor: input_system_keys_desc.handle(),
            input_mouse_keys: input_mouse_handle.value_handle,
            input_mouse_keys_cccd: input_mouse_handle.cccd_handle,
            input_mouse_keys_descriptor: input_mouse_desc.handle(),
            input_vial_keys: input_vial_handle.value_handle,
            input_vial_keys_cccd: input_vial_handle.cccd_handle,
            input_vial_keys_descriptor: input_vial_desc.handle(),
            output_vial: output_vial_handle.value_handle,
            output_vial_descriptor: output_vial_desc.handle(),
        })
    }

    pub fn on_write(&self, _conn: &Connection, handle: u16, data: &[u8]) {
        if handle == self.input_keyboard_cccd {
            info!("HID input keyboard notify: {:?}", data);
        } else if handle == self.output_keyboard {
            // Fires if a keyboard output is changed - e.g. the caps lock LED
            info!("HID output keyboard: {:?}", data);
        } else if handle == self.input_media_keys_cccd {
            info!("HID input media keys: {:?}", data);
        }
    }

    pub(crate) fn send_ble_keyboard_report(&self, conn: &Connection, data: &[u8]) {
        gatt_server::notify_value(conn, self.input_keyboard, data)
            .map_err(|e| error!("send keyboard report error: {}", e))
            .ok();
    }

    pub(crate) fn send_ble_media_report(&self, conn: &Connection, data: &[u8]) {
        gatt_server::notify_value(conn, self.input_media_keys, data)
            .map_err(|e| error!("send keyboard report error: {}", e))
            .ok();
    }
}
