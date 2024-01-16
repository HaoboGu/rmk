use core::sync::atomic::{AtomicBool, Ordering};
use embassy_usb::{
    class::hid::{Config, HidReaderWriter, ReportId, RequestHandler, State},
    control::OutResponse,
    driver::Driver,
    Builder, Handler, UsbDevice,
};
use log::{error, info};
use static_cell::StaticCell;
use usbd_hid::descriptor::{
    KeyboardReport, MediaKeyboardReport, SerializedDescriptor, SystemControlReport,
};

pub mod descriptor;

use crate::usb::descriptor::ViaReport;

static SUSPENDED: AtomicBool = AtomicBool::new(false);

// TODO: Use a composite hid device for Keyboard + Mouse + System control + Consumer control
// In this case, report id should be used.
// The keyboard usb device should have 3 hid instances:
// 1. Boot keyboard: 1 endpoint in
// 2. Other: Mouse + System control + Consumer control: 1 endpoint in
// 3. Via: used to communicate with via: 2 endpoints(in/out)
pub struct KeyboardUsbDevice<'d, D: Driver<'d>> {
    pub device: UsbDevice<'d, D>,
    pub keyboard_hid: HidReaderWriter<'d, D, 1, 8>,
    pub other_hid: HidReaderWriter<'d, D, 1, 8>,
    pub via_hid: HidReaderWriter<'d, D, 32, 32>,
}

impl<D: Driver<'static>> KeyboardUsbDevice<'static, D> {
    pub fn new(driver: D) -> Self {
        // Create embassy-usb Config
        let mut usb_config = embassy_usb::Config::new(0xc0de, 0xcafe);
        usb_config.manufacturer = Some("rmk");
        usb_config.product = Some("demo keyboard");
        usb_config.serial_number = Some("00000001");

        // Create embassy-usb DeviceBuilder using the driver and config.
        static DEVICE_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static MSOS_DESC: StaticCell<[u8; 128]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();

        // UsbDevice builder
        let mut builder = Builder::new(
            driver,
            usb_config,
            &mut DEVICE_DESC.init([0; 256])[..],
            &mut CONFIG_DESC.init([0; 256])[..],
            &mut BOS_DESC.init([0; 256])[..],
            &mut MSOS_DESC.init([0; 128])[..],
            &mut CONTROL_BUF.init([0; 128])[..],
        );

        static device_handler: StaticCell<MyDeviceHandler> = StaticCell::new();
        builder.handler(device_handler.init(MyDeviceHandler::new()));

        // Create classes on the builder.
        static request_handler: MyRequestHandler = MyRequestHandler {};

        // Initialize two hid interfaces: keyboard & via
        let keyboard_hid_config = Config {
            report_descriptor: KeyboardReport::desc(),
            request_handler: Some(&request_handler),
            poll_ms: 60,
            max_packet_size: 64,
        };
        static KEYBOARD_HID_STATE: StaticCell<State> = StaticCell::new();
        let keyboard_hid: HidReaderWriter<'_, D, 1, 8> = HidReaderWriter::new(
            &mut builder,
            KEYBOARD_HID_STATE.init(State::new()),
            keyboard_hid_config,
        );

        let other_hid_config = Config {
            report_descriptor: SystemControlReport::desc(),
            request_handler: Some(&request_handler),
            poll_ms: 60,
            max_packet_size: 64,
        };
        static OTHER_HID_STATE: StaticCell<State> = StaticCell::new();
        let other_hid: HidReaderWriter<'_, D, 1, 8> = HidReaderWriter::new(
            &mut builder,
            OTHER_HID_STATE.init(State::new()),
            other_hid_config,
        );

        let via_config = Config {
            report_descriptor: ViaReport::desc(),
            request_handler: Some(&request_handler),
            poll_ms: 60,
            max_packet_size: 64,
        };
        static VIA_STATE: StaticCell<State> = StaticCell::new();
        let via_hid: HidReaderWriter<'_, D, 32, 32> =
            HidReaderWriter::new(&mut builder, VIA_STATE.init(State::new()), via_config);

        // Build usb device
        let usb = builder.build();
        return Self {
            device: usb,
            keyboard_hid,
            other_hid,
            via_hid,
        };
    }

    pub async fn run() {}

    /// Send keyboard hid report
    pub async fn send_keyboard_report(&mut self, report: &KeyboardReport) {
        match self.keyboard_hid.write_serialize(report).await {
            Ok(_) => {}
            Err(e) => error!("Send keyboard report error: {:?}", e),
        };
    }

    /// Read via report, returns the length of the report, 0 if no report is available.
    pub async fn read_via_report(&mut self, report: &mut ViaReport) -> usize {
        // Use output_data: host to device data
        match self.via_hid.read(&mut report.output_data).await {
            Ok(l) => l,
            Err(e) => {
                error!("Read via report error: {:?}", e);
                0
            }
        }
    }

    pub async fn send_via_report(&mut self, report: &ViaReport) {
        match self.via_hid.write_serialize(report).await {
            Ok(_) => {}
            Err(e) => {
                error!("Send via report error: {:?}", e);
            }
        }
    }

    /// Send consumer control report, commonly used in keyboard media control
    pub async fn send_consumer_control_report(&mut self, report: &MediaKeyboardReport) {
        match self.other_hid.write_serialize(report).await {
            Ok(_) => {}
            Err(e) => {
                error!("Send via report error: {:?}", e);
            }
        }
    }

    /// Send system control report
    pub async fn send_system_control_report(&mut self, report: &SystemControlReport) {
        match self.other_hid.write_serialize(report).await {
            Ok(_) => {}
            Err(e) => {
                error!("Send via report error: {:?}", e);
            }
        }
    }
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {:?}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}

struct MyDeviceHandler {
    configured: AtomicBool,
}

impl MyDeviceHandler {
    fn new() -> Self {
        MyDeviceHandler {
            configured: AtomicBool::new(false),
        }
    }
}

impl Handler for MyDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        self.configured.store(false, Ordering::Relaxed);
        SUSPENDED.store(false, Ordering::Release);
        if enabled {
            info!("Device enabled");
        } else {
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        self.configured.store(false, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        self.configured.store(false, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        if configured {
            info!(
                "Device configured, it may now draw up to the configured current limit from Vbus."
            )
        } else {
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }

    fn suspended(&mut self, suspended: bool) {
        if suspended {
            info!("Device suspended, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled).");
            SUSPENDED.store(true, Ordering::Release);
        } else {
            SUSPENDED.store(false, Ordering::Release);
            if self.configured.load(Ordering::Relaxed) {
                info!(
                    "Device resumed, it may now draw up to the configured current limit from Vbus"
                );
            } else {
                info!("Device resumed, the Vbus current limit is 100mA");
            }
        }
    }
}
