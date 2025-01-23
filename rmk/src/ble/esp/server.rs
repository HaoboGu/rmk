extern crate alloc;
use alloc::sync::Arc;
use embassy_futures::block_on;
use embassy_time::Timer;
use esp32_nimble::{
    enums::{AuthReq, SecurityIOCap},
    utilities::{mutex::Mutex, BleUuid},
    BLEAdvertisementData, BLECharacteristic, BLEDevice, BLEHIDDevice, BLEServer, NimbleProperties,
};
use ssmarshal::serialize;
use usbd_hid::descriptor::SerializedDescriptor as _;

use crate::{
    ble::{
        descriptor::{BleCompositeReportType, BleKeyboardReport},
        device_info::VidSource,
    },
    channel::{KEYBOARD_REPORT_CHANNEL, LED_CHANNEL},
    config::KeyboardUsbConfig,
    hid::{HidError, HidReaderTrait, HidWriterTrait, Report},
    light::LedIndicator,
    usb::descriptor::ViaReport,
};

use super::VIAL_READ_CHANNEL;

pub(crate) struct BleKeyboardWriter {
    pub(crate) keyboard_handle: Arc<Mutex<BLECharacteristic>>,
    pub(crate) media_handle: Arc<Mutex<BLECharacteristic>>,
    pub(crate) system_control_handle: Arc<Mutex<BLECharacteristic>>,
    pub(crate) mouse_handle: Arc<Mutex<BLECharacteristic>>,
}

impl HidWriterTrait for BleKeyboardWriter {
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
                self.write(&self.keyboard_handle, &buf).await?;
                Ok(n)
            }
            Report::MouseReport(mouse_report) => {
                let mut buf = [0u8; 5];
                let n = serialize(&mut buf, &mouse_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.write(&self.mouse_handle, &buf).await?;
                Ok(n)
            }
            Report::MediaKeyboardReport(media_keyboard_report) => {
                let mut buf = [0u8; 2];
                let n = serialize(&mut buf, &media_keyboard_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.write(&self.media_handle, &buf).await?;
                Ok(n)
            }
            Report::SystemControlReport(system_control_report) => {
                let mut buf = [0u8; 2];
                let n = serialize(&mut buf, &system_control_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.write(&self.system_control_handle, &buf).await?;
                Ok(n)
            }
        }
    }
}

impl BleKeyboardWriter {
    async fn write(
        &self,
        handle: &Arc<Mutex<BLECharacteristic>>,
        report: &[u8],
    ) -> Result<(), HidError> {
        debug!("BLE notify {} {=[u8]:#X}", report.len(), report);
        handle.lock().set_value(report).notify();
        Timer::after_millis(7).await;
        Ok(())
    }
}

pub(crate) struct BleLedReader {
    pub(crate) keyboard_output_handle: Arc<Mutex<BLECharacteristic>>,
}

impl HidReaderTrait for BleLedReader {
    type ReportType = LedIndicator;

    // ESP BLE provides only a blocking callback function for reading data.
    // So we use a channel to do async read
    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        Ok(LED_CHANNEL.receive().await)
    }
}

impl BleLedReader {
    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, HidError> {
        self.keyboard_output_handle
            .lock()
            .on_read(|characteristic, _conn| {
                let v = characteristic.value_mut();
                info!("led on_read!, {} {=[u8]:#X}", v.len(), v.as_slice());
                let led_indicator = LedIndicator::from_bits(v.as_slice()[0]);
                if let Err(e) = LED_CHANNEL.try_send(led_indicator) {
                    warn!("LED channel full: {:?}", e);
                }
            });
        Ok(1)
    }
}

pub(crate) struct BleVialReaderWriter {
    // Read vial data from host via vial_output_handle
    pub(crate) vial_output_handle: Arc<Mutex<BLECharacteristic>>,
    // Writer vial data to host via vial_input_handle
    pub(crate) vial_input_handle: Arc<Mutex<BLECharacteristic>>,
}

impl HidReaderTrait for BleVialReaderWriter {
    type ReportType = ViaReport;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let v = VIAL_READ_CHANNEL.receive().await;

        Ok(ViaReport {
            input_data: v,
            output_data: [0u8; 32],
        })
    }
}

impl HidWriterTrait for BleVialReaderWriter {
    type ReportType = ViaReport;

    async fn get_report(&mut self) -> Self::ReportType {
        todo!()
    }

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        let mut buf = [0u8; 32];
        let n = serialize(&mut buf, &report).map_err(|_| HidError::ReportSerializeError)?;
        self.write(&buf).await?;
        Ok(n)
    }
}

impl BleVialReaderWriter {
    async fn write(&self, report: &[u8]) -> Result<(), HidError> {
        self.vial_input_handle.lock().set_value(&report).notify();
        Timer::after_millis(7).await;
        Ok(())
    }

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, HidError> {
        self.vial_output_handle
            .lock()
            .on_read(|characteristic, _conn| {
                let v = characteristic.value_mut();
                info!("vial on_read!, {} {=[u8]:#X}", v.len(), v.as_slice());
                let data = unsafe { *(v.as_slice().as_ptr() as *const [u8; 32]) };
                if let Err(_e) = VIAL_READ_CHANNEL.try_send(data) {
                    error!("Vial output channel full: ");
                }
            });
        Ok(32)
    }
}

// BLE HID keyboard server
pub(crate) struct BleServer {
    pub(crate) server: &'static mut BLEServer,
    pub(crate) input_keyboard: Arc<Mutex<BLECharacteristic>>,
    pub(crate) output_keyboard: Arc<Mutex<BLECharacteristic>>,
    pub(crate) input_media_keys: Arc<Mutex<BLECharacteristic>>,
    pub(crate) input_system_keys: Arc<Mutex<BLECharacteristic>>,
    pub(crate) input_mouse_keys: Arc<Mutex<BLECharacteristic>>,
    pub(crate) input_vial: Arc<Mutex<BLECharacteristic>>,
    pub(crate) output_vial: Arc<Mutex<BLECharacteristic>>,
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
            .create_characteristic(BleUuid::from_uuid16(0x2a50), NimbleProperties::READ)
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
            .create_characteristic(BleUuid::from_uuid16(0x2a50), NimbleProperties::READ)
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

    pub(crate) fn get_led_reader(&self) -> BleLedReader {
        BleLedReader {
            keyboard_output_handle: self.output_keyboard.clone(),
        }
    }

    pub(crate) fn get_keyboard_writer(&self) -> BleKeyboardWriter {
        BleKeyboardWriter {
            keyboard_handle: self.input_keyboard.clone(),
            media_handle: self.input_media_keys.clone(),
            system_control_handle: self.input_system_keys.clone(),
            mouse_handle: self.input_mouse_keys.clone(),
        }
    }

    pub(crate) fn get_vial_reader_writer(&self) -> BleVialReaderWriter {
        BleVialReaderWriter {
            vial_output_handle: self.output_vial.clone(),
            vial_input_handle: self.input_vial.clone(),
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

// pub(crate) struct VialReaderWriter<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper>
// {
//     pub(crate) receiver: Receiver<'ch, M, T, N>,
//     pub(crate) hid_writer: W,
// }

// impl<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper> ConnectionTypeWrapper
//     for VialReaderWriter<'ch, M, T, N, W>
// {
//     fn get_conn_type(&self) -> ConnectionType {
//         ConnectionType::Ble
//     }
// }

// impl<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper> HidReaderWrapper
//     for VialReaderWriter<'ch, M, T, N, W>
// {
//     async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError> {
//         let v = self.receiver.receive().await;
//         buf.copy_from_slice(as_bytes(&v));
//         Ok(as_bytes(&v).len())
//     }
// }

// impl<'ch, M: RawMutex, T: Sized, const N: usize, W: HidWriterWrapper> HidWriterWrapper
//     for VialReaderWriter<'ch, M, T, N, W>
// {
//     async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
//         self.hid_writer.write_serialize(r).await
//     }

//     async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
//         self.hid_writer.write(report).await
//     }
// }
