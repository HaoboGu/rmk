use nrf_softdevice::ble::{
    advertisement_builder::{
        AdvertisementDataType, Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload,
        ServiceList, ServiceUuid16,
    },
    Uuid,
};

use super::HidSecurityHandler;

/// Specification uuid used in keyboards
///
/// Full reference: https://www.bluetooth.com/specifications/assigned-numbers/
/// UUID details: https://bitbucket.org/bluetooth-SIG/public/src/main/assigned_numbers/uuids/service_uuids.yaml
pub enum BleSpecification {
    DeviceInformation = 0x180a,
    BatteryService = 0x180f,
}

/// Characteristics uuids used in keyboards
///
/// refernece: https://bitbucket.org/bluetooth-SIG/public/src/main/assigned_numbers/uuids/characteristic_uuids.yaml
pub enum BleCharacteristics {
    BatteryLevel = 0x2a19,
    ModelNumber = 0x2a24,
    SerialNumber = 0x2a25,
    FirmwareRevision = 0x2a26,
    HardwareRevision = 0x2a27,
    SoftwareRevision = 0x2a28,
    ManufacturerName = 0x2a29,
    PnpId = 0x2a50,
    // Characteristics of HID
    HidInfo = 0x2a4a,
    ReportMap = 0x2a4b,
    HidControlPoint = 0x2a4c,
    HidReport = 0x2a4d,
    ProtocolMode = 0x2a4e,
}

impl BleSpecification {
    pub fn uuid(self) -> Uuid {
        Uuid::new_16(self as u16)
    }
}

impl BleCharacteristics {
    pub fn uuid(self) -> Uuid {
        Uuid::new_16(self as u16)
    }
}

pub const KEYBOARD_ID: u8 = 0x01;
pub const MEDIA_KEYS_ID: u8 = 0x02;
// TODO: Customize ADV name
pub static ADV_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
    .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
    .services_16(
        ServiceList::Incomplete,
        &[
            ServiceUuid16::BATTERY,
            ServiceUuid16::HUMAN_INTERFACE_DEVICE,
        ],
    )
    .full_name("RMK")
    // Change the appearance (icon of the bluetooth device) to a keyboard
    .raw(AdvertisementDataType::APPEARANCE, &[0xC1, 0x03])
    .build();

pub static SCAN_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
    .services_16(
        ServiceList::Complete,
        &[
            ServiceUuid16::DEVICE_INFORMATION,
            ServiceUuid16::BATTERY,
            ServiceUuid16::HUMAN_INTERFACE_DEVICE,
        ],
    )
    .build();

pub static HID_SECURITY_HANDLER: HidSecurityHandler = HidSecurityHandler {};
