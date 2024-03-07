use super::descriptor::BleKeyboardReport;
use usbd_hid::descriptor::SerializedDescriptor;

#[nrf_softdevice::gatt_service(uuid = "1812")]
pub struct HidService2 {
    #[characteristic(
        uuid = "2A4D",
        security = "justworks",
        read,
        write,
        notify,
        value = "[0u8, 1u8]",
        descriptor(uuid = "2908", security = "justworks", value = "[0, 1]")
    )]
    pub input_report: [u8; 8],

    #[characteristic(
        uuid = "2A4A",
        security = "justworks",
        read,
        value = "[0x1, 0x1, 0x0, 0x03]"
    )]
    pub hid_info: u8, 

    #[characteristic(
        uuid = "2A4B",
        security = "justworks",
        read,
        value = "BleKeyboardReport::desc()",
    )]
    pub report_map: [u8; 71],

    #[characteristic(
        uuid = "2A4E",
        security = "justworks",
        read,
        write_without_response,
        value = "[1u8]",
    )]
    pub protocol_mode: [u8; 1],

    #[characteristic(
        uuid = "2A4C",
        security = "justworks",
        read,
        write_without_response,
        value = "[0u8]",
    )]
    pub hid_control: [u8; 1],
}

#[nrf_softdevice::gatt_service(uuid = "180f")]
pub struct BatteryService {
    #[characteristic(uuid = "2a19", security = "justworks", read, notify)]
    pub battery_level: u8,
}

#[nrf_softdevice::gatt_service(uuid = "180A")]
pub struct DeviceInformationService {}

#[nrf_softdevice::gatt_server]
pub struct BleServer2 {
    pub battery_service: BatteryService,
    pub device_information_service: DeviceInformationService,
    pub hid_service: HidService2,
}

