extern crate alloc;
use alloc::sync::Arc;
use esp32_nimble::{
    enums::{AuthReq, SecurityIOCap},
    utilities::mutex::Mutex,
    BLEAdvertisementData, BLECharacteristic, BLEDevice, BLEHIDDevice, BLEServer,
};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor as _};

use crate::{
    ble::{
        descriptor::{BleCompositeReportType, BleKeyboardReport},
        device_info::VidSource,
    },
    config::KeyboardUsbConfig,
};

pub(crate) struct BleServer {
    server: &'static mut BLEServer,
    input_keyboard: Arc<Mutex<BLECharacteristic>>,
    output_keyboard: Arc<Mutex<BLECharacteristic>>,
    input_media_keys: Arc<Mutex<BLECharacteristic>>,
    input_system_keys: Arc<Mutex<BLECharacteristic>>,
    input_mouse_keys: Arc<Mutex<BLECharacteristic>>,
    input_vial: Arc<Mutex<BLECharacteristic>>,
    output_vial: Arc<Mutex<BLECharacteristic>>,
}

impl BleServer {
    pub(crate) fn new(usb_config: KeyboardUsbConfig) -> Self {
        let device = BLEDevice::take();
        device
            .security()
            .set_auth(AuthReq::all())
            .set_io_cap(SecurityIOCap::NoInputNoOutput);
        let server = device.get_server();
        let mut hid = BLEHIDDevice::new(server);
        hid.manufacturer(usb_config.manufacturer.unwrap_or("RMK Keyboard"));
        let input_keyboard = hid.input_report(BleCompositeReportType::Keyboard as u8);
        let output_keyboard = hid.output_report(BleCompositeReportType::Keyboard as u8);
        let input_media_keys = hid.input_report(BleCompositeReportType::Media as u8);
        let input_system_keys = hid.input_report(BleCompositeReportType::System as u8);
        let input_mouse_keys = hid.input_report(BleCompositeReportType::Mouse as u8);
        let input_vial = hid.input_report(BleCompositeReportType::Vial as u8);
        let output_vial = hid.output_report(BleCompositeReportType::Vial as u8);

        hid.pnp(
            VidSource::UsbIF as u8,
            usb_config.vid,
            usb_config.pid,
            0x0000,
        );
        hid.hid_info(0x00, 0x03);
        hid.report_map(BleKeyboardReport::desc());
        hid.set_battery_level(100);
        let ble_advertising = device.get_advertising();
        ble_advertising
            .lock()
            .scan_response(false)
            .set_data(
                BLEAdvertisementData::new()
                    .name("ESP32 Keyboard")
                    .appearance(0x03C1)
                    .add_service_uuid(hid.hid_service().lock().uuid()),
            )
            .unwrap();
        ble_advertising.lock().start().unwrap();

        Self {
            server,
            input_keyboard,
            output_keyboard,
            input_media_keys,
            input_system_keys,
            input_mouse_keys,
            input_vial,
            output_vial,
        }
    }

    pub(crate) fn connected(&self) -> bool {
        self.server.connected_count() > 0
    }

    pub(crate) fn send_ble_keyboard_report(&mut self, report: KeyboardReport) {
        self.input_keyboard.lock().set_from(&report).notify();
        esp_idf_hal::delay::Ets::delay_ms(7);
    }
}
