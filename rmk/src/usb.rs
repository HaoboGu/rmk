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

use crate::via::descriptor::ViaReport;

static SUSPENDED: AtomicBool = AtomicBool::new(false);

// TODO: Use a composite hid device for Keyboard + Mouse + System control + Consumer control
// In this case, report id should be used.
// The keyboard usb device should have 3 hid instances:
// 1. Boot keyboard: 1 endpoint in
// 2. Composite keyboard: Keyboard + Mouse + System control + Consumer control: 1 endpoint in
// 3. Raw hid communication: used to communicate with via: 2 endpoints(in/out)
pub struct KeyboardUsbDevice<'d, D: Driver<'d>> {
    pub device: UsbDevice<'d, D>,
    pub hid: HidReaderWriter<'d, D, 1, 8>,
    // pub via_hid: HidReaderWriter<'d, D, 1, 8>,
    // pub system_control_hid: HidReaderWriter<'d, D, 1, 8>,
    // pub consumer_control_hid: HidReaderWriter<'d, D, 1, 8>,
}

static STATE: StaticCell<State> = StaticCell::new();

impl<'d, D: Driver<'d>> KeyboardUsbDevice<'d, D> {
    pub fn new(driver: D, state: &'d mut State<'d>) -> Self {
        // Create embassy-usb Config
        let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Embassy");
        config.product = Some("rmkkeyboard");
        config.serial_number = Some("12345678");

        // Create embassy-usb DeviceBuilder using the driver and config.
        static DEVICE_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static MSOS_DESC: StaticCell<[u8; 128]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();

        let mut builder = Builder::new(
            driver,
            config,
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
        let config = Config {
            report_descriptor: KeyboardReport::desc(),
            request_handler: Some(&request_handler),
            poll_ms: 60,
            max_packet_size: 64,
        };
        let hid: HidReaderWriter<'d, D, 1, 8> =
            HidReaderWriter::<_, 1, 8>::new(&mut builder, state, config);
        // let via_hid: HidReaderWriter<'d, D, 1, 8> =
        //     HidReaderWriter::<_, 1, 8>::new(&mut builder, state, config);
        // let consumer_control_hid: HidReaderWriter<'d, D, 1, 8> =
        //     HidReaderWriter::<_, 1, 8>::new(&mut builder, state, config);
        // let system_control_hid: HidReaderWriter<'d, D, 1, 8> =
        //     HidReaderWriter::<_, 1, 8>::new(&mut builder, state, config);
        let usb = builder.build();
        return Self {
            device: usb,
            hid,
            // via_hid,
            // system_control_hid,
            // consumer_control_hid,
        };
    }

    pub async fn run() {}

    /// Send keyboard hid report
    pub async fn send_keyboard_report(&mut self, report: &KeyboardReport) {
        match self.hid.write_serialize(report).await {
            Ok(()) => {}
            Err(e) => error!("Send keyboard report error: {:?}", e),
        };
    }

    /// Read via report, returns the length of the report, 0 if no report is available.
    pub fn read_via_report(&mut self, _report: &mut ViaReport) -> usize {
        // Use output_data: host to device data
        // match self.via_hid.write_serialize(&mut report.output_data).await {
        //     Ok(l) => l,
        //     Err(e) => {
        //         error!("Read via report error: {:?}", e);
        //         0
        //     }
        // }
        0
    }

    pub fn send_via_report(&self, _report: &ViaReport) -> usize {
        // Use output_data: host to device data
        // match self.via_hid.push_input(report) {
        //     Ok(l) => l,
        //     Err(e) => {
        //         error!("Send via report error: {:?}", e);
        //         0
        //     }
        // }
        0
    }

    /// Send consumer control report, commonly used in keyboard media control
    pub fn send_consumer_control_report(&self, _report: &MediaKeyboardReport) {
        // match self.consumer_control_hid.push_input(report) {
        //     Ok(_) => (),
        //     Err(e) => error!("Send consumer control report error: {:?}", e),
        // }
    }

    /// Send system control report
    pub fn send_system_control_report(&self, _report: &SystemControlReport) {
        // match self.system_control_hid.push_input(report) {
        //     Ok(_) => (),
        //     Err(e) => error!("Send system control report error: {:?}", e),
        // }
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
