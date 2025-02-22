extern crate alloc;
use alloc::sync::Arc;
use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Receiver};
use embassy_time::Timer;
use esp32_nimble::{
    enums::{AuthReq, SecurityIOCap},
    utilities::{mutex::Mutex, BleUuid},
    BLEAdvertisementData, BLECharacteristic, BLEDevice, BLEHIDDevice, BLEServer, NimbleProperties,
};
use usbd_hid::descriptor::{AsInputReport, SerializedDescriptor as _};

use crate::{
    ble::{
        as_bytes,
        descriptor::{BleCompositeReportType, BleKeyboardReport},
        device_info::VidSource,
    },
    config::KeyboardUsbConfig,
    hid::{ConnectionType, ConnectionTypeWrapper, HidError, HidReaderWrapper, HidWriterWrapper},
    usb::descriptor::ViaReport,
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
        match serialize(&mut buf, r) {
            Ok(n) => self.write(&buf[0..n]).await,
            Err(_) => Err(HidError::ReportSerializeError),
        }
    }

    async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
        debug!("BLE notify {} {=[u8]:#X}", report.len(), report);
        self.lock().set_value(report).notify();
        Timer::after_millis(7).await;
        Ok(())
    }
}

impl HidReaderWrapper for BleHidReader {
    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, HidError> {
        self.lock().on_read(|characteristic, _conn| {
            let v = characteristic.value_mut();
            info!("on_read!, {} {=[u8]:#X}", v.len(), v.as_slice());
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
        let keyboard_name = usb_config.product_name;
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
            info!("Disconnected!");
        });
        let mut hid = BLEHIDDevice::new(server);
        hid.manufacturer(usb_config.manufacturer);
        block_on(server.get_service(BleUuid::from_uuid16(0x180a)))
            .unwrap()
            .lock()
            .create_characteristic(BleUuid::from_uuid16(0x2a25), NimbleProperties::READ)
            .lock()
            .set_value(usb_config.serial_number.as_bytes());

        let input_keyboard = hid.input_report(BleCompositeReportType::Keyboard as u8);
        let output_keyboard = hid.output_report(BleCompositeReportType::Keyboard as u8);
        let input_media_keys = hid.input_report(BleCompositeReportType::Media as u8);
        let input_system_keys = hid.input_report(BleCompositeReportType::System as u8);
        let input_mouse_keys = hid.input_report(BleCompositeReportType::Mouse as u8);

        hid.pnp(
            VidSource::UsbIF as u8,
            usb_config.vid,
            usb_config.pid,
            0x0000,
        );
        hid.set_battery_level(80);
        hid.hid_info(0x00, 0x03);
        hid.report_map(BleKeyboardReport::desc());

        let mut vial_hid = BLEHIDDevice::new(server);
        vial_hid.manufacturer(usb_config.manufacturer);
        block_on(server.get_service(BleUuid::from_uuid16(0x180a)))
            .unwrap()
            .lock()
            .create_characteristic(BleUuid::from_uuid16(0x2a25), NimbleProperties::READ)
            .lock()
            .set_value(usb_config.serial_number.as_bytes());
        let input_vial = vial_hid.input_report(0);
        let output_vial = vial_hid.output_report(0);

        vial_hid.pnp(
            VidSource::UsbIF as u8,
            usb_config.vid,
            usb_config.pid,
            0x0000,
        );
        vial_hid.hid_info(0x00, 0x03);
        vial_hid.report_map(ViaReport::desc());

        let ble_advertising = device.get_advertising();
        if let Err(e) = ble_advertising.lock().scan_response(false).set_data(
            BLEAdvertisementData::new()
                .name(keyboard_name)
                .appearance(0x03C1)
                .add_service_uuid(hid.hid_service().lock().uuid())
                .add_service_uuid(vial_hid.hid_service().lock().uuid()),
        ) {
            error!("BLE advertising error, error code: {}", e.code());
        }

        if let Err(e) = ble_advertising.lock().start() {
            error!("BLE advertising start error: {}", e.code());
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

pub(crate) struct VialReaderWriter<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper>
{
    pub(crate) receiver: Receiver<'ch, M, T, N>,
    pub(crate) hid_writer: W,
}

impl<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper> ConnectionTypeWrapper
    for VialReaderWriter<'ch, M, T, N, W>
{
    fn get_conn_type(&self) -> ConnectionType {
        ConnectionType::Ble
    }
}

impl<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper> HidReaderWrapper
    for VialReaderWriter<'ch, M, T, N, W>
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError> {
        let v = self.receiver.receive().await;
        buf.copy_from_slice(as_bytes(&v));
        Ok(as_bytes(&v).len())
    }
}

impl<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper> HidWriterWrapper
    for VialReaderWriter<'ch, M, T, N, W>
{
    async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
        self.hid_writer.write_serialize(r).await
    }

    async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
        self.hid_writer.write(report).await
    }
}
