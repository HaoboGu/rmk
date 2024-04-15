extern crate alloc;
use alloc::sync::Arc;
use defmt::{error, info, warn};
use embassy_time::Timer;
use esp32_nimble::{
    enums::{AuthReq, SecurityIOCap},
    utilities::mutex::Mutex,
    BLEAdvertisementData, BLECharacteristic, BLEDevice, BLEHIDDevice, BLEServer,
};
use usbd_hid::descriptor::{AsInputReport, SerializedDescriptor as _};

use crate::{
    ble::{
        descriptor::{BleCompositeReportType, BleKeyboardReport},
        device_info::VidSource,
    },
    config::KeyboardUsbConfig,
    hid::{ConnectionType, ConnectionTypeWrapper, HidError, HidReaderWrapper, HidWriterWrapper},
};

type BleHidWriter = Arc<Mutex<BLECharacteristic>>;
type BleHidReader = Arc<Mutex<BLECharacteristic>>;

impl ConnectionTypeWrapper for BleHidWriter {
    fn get_conn_type(&self) -> ConnectionType {
        ConnectionType::Ble
    }
}

impl HidWriterWrapper for BleHidWriter {
    async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
        use ssmarshal::serialize;
        let mut buf: [u8; 32] = [0; 32];
        match serialize(&mut buf, &r) {
            Ok(n) => self.write(&buf[0..n]).await,
            Err(_) => Err(HidError::ReportSerializeError),
        }
    }

    async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
        self.lock().set_value(report).notify();
        esp_idf_svc::hal::delay::Ets::delay_ms(7);
        Ok(())
    }
}

// FIXME: ESP BLE HID Reader
impl HidReaderWrapper for BleHidReader {
    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, HidError> {
        self.lock().on_read(|a, _| {
            info!("on_read!, {} {=[u8]:#X}", a.len(), a.value());
        });
        Ok(1)
    }
}

// BLE HID keyboard server
pub(crate) struct BleServer {
    pub(crate) server: &'static mut BLEServer,
    pub(crate) input_keyboard: BleHidWriter,
    pub(crate) output_keyboard: BleHidReader,
    pub(crate) input_media_keys: BleHidWriter,
    pub(crate) input_system_keys: BleHidWriter,
    pub(crate) input_mouse_keys: BleHidWriter,
    pub(crate) input_vial: BleHidWriter,
    pub(crate) output_vial: BleHidReader,
}

impl BleServer {
    pub(crate) fn new(usb_config: KeyboardUsbConfig) -> Self {
        let keyboard_name = usb_config.product_name.unwrap_or("RMK Keyboard");
        let device = BLEDevice::take();
        BLEDevice::set_device_name(keyboard_name).ok();
        device
            .security()
            .set_auth(AuthReq::all())
            .set_io_cap(SecurityIOCap::NoInputNoOutput)
            .resolve_rpa();
        let server = device.get_server();
        // Set disconnected callback
        server.on_disconnect(|_, r| {
            if let Err(e) = r {
                warn!("BLE disconnected, error code: {}", e.code());
            }
        });
        let mut hid = BLEHIDDevice::new(server);
        hid.manufacturer(usb_config.manufacturer.unwrap_or("Haobo"));
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
        hid.set_battery_level(80);
        hid.hid_info(0x00, 0x03);
        hid.report_map(BleKeyboardReport::desc());

        let ble_advertising = device.get_advertising();
        match ble_advertising
            .lock()
            .scan_response(false)
            .set_data(
                BLEAdvertisementData::new()
                    .name(keyboard_name)
                    .appearance(0x03C1)
                    .add_service_uuid(hid.hid_service().lock().uuid()),
            )
            .and_then(|_| ble_advertising.lock().start())
        {
            Ok(_) => (),
            Err(e) => error!("BLE advertising error, error code: {}", e.code()),
        }

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

    pub(crate) async fn wait_for_connection(&mut self) {
        loop {
            // Check connection status every 100 ms
            Timer::after_millis(100).await;
            if self.server.connected_count() > 0 {
                break;
            }
        }
    }

    pub(crate) async fn wait_for_disconnection(server: &'static mut BLEServer) {
        loop {
            // Check connection status every 500 ms
            Timer::after_millis(500).await;
            if server.connected_count() == 0 {
                break;
            }
        }
    }
}

impl ConnectionTypeWrapper for BleServer {
    fn get_conn_type(&self) -> crate::hid::ConnectionType {
        ConnectionType::Ble
    }
}

impl HidWriterWrapper for BleServer {
    async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
        self.input_keyboard.lock().set_from(r).notify();
        esp_idf_svc::hal::delay::Ets::delay_ms(7);
        Ok(())
    }

    async fn write(&mut self, report: &[u8]) -> Result<(), crate::hid::HidError> {
        self.input_keyboard.lock().set_value(report).notify();
        esp_idf_svc::hal::delay::Ets::delay_ms(7);
        Ok(())
    }
}
