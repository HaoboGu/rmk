use super::spec::{BleCharacteristics, BleDescriptor, BLE_HID_SERVICE_UUID};
use crate::{
    ble::{
        descriptor::{BleCompositeReportType, BleKeyboardReport},
        HidError,
    }, channel::LED_CHANNEL, hid::HidReaderTrait, light::LedIndicator
};
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

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct HidService {
    hid_info: u16,
    report_map: u16,
    hid_control: u16,
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
}

impl HidService {
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
                Attribute::new(BleKeyboardReport::desc()).security(SecurityMode::JustWorks),
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

        let mut input_keyboard = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 8]).security(SecurityMode::JustWorks),
            Metadata::new(Properties::new().read().notify()),
        )?;
        let input_keyboard_desc = input_keyboard.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Keyboard as u8, 1u8]) // First is report ID, second is in/out
                .security(SecurityMode::JustWorks),
        )?;
        let input_keyboard_handle = input_keyboard.build();

        let mut output_keyboard = service_builder.add_characteristic(
            BleCharacteristics::HidReport.uuid(),
            Attribute::new([0u8; 1]).security(SecurityMode::JustWorks),
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
            Metadata::new(Properties::new().read().notify()),
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
            Metadata::new(Properties::new().read().notify()),
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
            Metadata::new(Properties::new().read().notify()),
        )?;
        let input_mouse_desc = input_mouse.add_descriptor(
            BleDescriptor::ReportReference.uuid(),
            Attribute::new([BleCompositeReportType::Mouse as u8, 1u8])
                .security(SecurityMode::JustWorks),
        )?;
        let input_mouse_handle = input_mouse.build();

        let _service_handle = service_builder.build();

        Ok(HidService {
            hid_info: hid_info_handle.value_handle,
            report_map: report_map_handle.value_handle,
            hid_control: hid_control_handle.value_handle,
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
        })
    }

    pub(crate) fn send_ble_keyboard_report(&self, conn: &Connection, data: &[u8]) {
        gatt_server::notify_value(conn, self.input_keyboard, data)
            .map_err(|e| error!("send keyboard report error: {:?}", e))
            .ok();
    }

    pub(crate) fn send_ble_media_report(&self, conn: &Connection, data: &[u8]) {
        gatt_server::notify_value(conn, self.input_media_keys, data)
            .map_err(|e| error!("send keyboard report error: {:?}", e))
            .ok();
    }
}

impl gatt_server::Service for HidService {
    type Event = HidServiceEvent;

    fn on_write(&self, handle: u16, data: &[u8]) -> Option<Self::Event> {
        if handle == self.input_keyboard_cccd {
            Some(HidServiceEvent::InputKeyboardCccdWrite)
        } else if handle == self.input_media_keys_cccd {
            Some(HidServiceEvent::InputMediaKeyCccdWrite)
        } else if handle == self.input_mouse_keys_cccd {
            Some(HidServiceEvent::InputMouseKeyCccdWrite)
        } else if handle == self.input_system_keys_cccd {
            Some(HidServiceEvent::InputSystemKeyCccdWrite)
        } else if handle == self.output_keyboard {
            // Fires if a keyboard output is changed - e.g. the caps lock LED
            let led_indicator = LedIndicator::from_bits(data[0]);
            info!("HID output keyboard: {:?}", led_indicator);
            // Retry 3 times in case the channel is full(which is really rare)
            for _i in 0..3 {
                match LED_CHANNEL.try_send(led_indicator) {
                    Ok(_) => break,
                    Err(e) => warn!("LED channel full, retrying: {:?}", e),
                }
            }
            Some(HidServiceEvent::OutputKeyboard)
        } else {
            None
        }
    }
}

#[allow(unused)]
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum HidServiceEvent {
    InputKeyboardCccdWrite,
    InputMediaKeyCccdWrite,
    InputMouseKeyCccdWrite,
    InputSystemKeyCccdWrite,
    OutputKeyboard,
}

pub(crate) struct BleLedReader {}

impl HidReaderTrait for BleLedReader {
    type ReportType = LedIndicator;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        Ok(LED_CHANNEL.receive().await)
    }
}

pub(crate) struct BleKeyboardWriter<'a> {
    conn: &'a Connection,
    keyboard_handle: u16,
    media_handle: u16,
    system_control_handle: u16,
    mouse_handle: u16,
}

impl<'a> BleKeyboardWriter<'a> {
    pub(crate) fn new(
        conn: &'a Connection,
        keyboard_handle: u16,
        media_handle: u16,
        system_control_handle: u16,
        mouse_handle: u16,
    ) -> Self {
        Self {
            conn,
            keyboard_handle,
            media_handle,
            system_control_handle,
            mouse_handle,
        }
    }
    async fn write(&mut self, handle: u16, report: &[u8]) -> Result<(), HidError> {
        gatt_server::notify_value(self.conn, handle, report).map_err(|e| {
            error!("Send ble report error: {}", e);
            match e {
                gatt_server::NotifyValueError::Disconnected => HidError::BleDisconnected,
                gatt_server::NotifyValueError::Raw(_) => HidError::BleRawError,
            }
        })
    }
}

impl HidWriterTrait for BleKeyboardWriter<'_> {
    type ReportType = Report;

    async fn get_report(&mut self) -> Self::ReportType {
        KEYBOARD_REPORT_CHANNEL.receive().await
    }

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        match report {
            Report::KeyboardReport(keyboard_report) => {
                let mut buf = [0u8; 8];
                let n = serialize(&mut buf, &keyboard_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.write(self.keyboard_handle, &buf).await?;
                Ok(n)
            }
            Report::MouseReport(mouse_report) => {
                let mut buf = [0u8; 5];
                let n = serialize(&mut buf, &mouse_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.write(self.mouse_handle, &buf).await?;
                Ok(n)
            }
            Report::MediaKeyboardReport(media_keyboard_report) => {
                let mut buf = [0u8; 2];
                let n = serialize(&mut buf, &media_keyboard_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.write(self.media_handle, &buf).await?;
                Ok(n)
            }
            Report::SystemControlReport(system_control_report) => {
                let mut buf = [0u8; 2];
                let n = serialize(&mut buf, &system_control_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.write(self.system_control_handle, &buf).await?;
                Ok(n)
            }
        }
    }
}
