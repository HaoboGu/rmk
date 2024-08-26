use nrf_softdevice::ble::Uuid;

/// HID service uuid defined in BLE protocol
pub const BLE_HID_SERVICE_UUID: Uuid = Uuid::new_16(0x1812);

/// Specification uuid used in keyboards
///
/// Full reference: https://www.bluetooth.com/specifications/assigned-numbers/
/// UUID details: https://bitbucket.org/bluetooth-SIG/public/src/main/assigned_numbers/uuids/service_uuids.yaml
pub enum BleSpecification {
    DeviceInformation = 0x180a,
    BatteryService = 0x180f,
    HidService = 0x1812,
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

pub enum BleDescriptor {
    ReportReference = 0x2908,
}

impl BleDescriptor {
    pub fn uuid(self) -> Uuid {
        Uuid::new_16(self as u16)
    }
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
