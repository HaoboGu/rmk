pub(crate) mod descriptor;

use core::sync::atomic::{AtomicBool, Ordering};
use defmt::info;
use embassy_usb::{
    class::hid::{Config, HidReader, HidReaderWriter, HidWriter, ReportId, RequestHandler, State},
    control::OutResponse,
    driver::Driver,
    Builder, Handler, UsbDevice,
};
use static_cell::StaticCell;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

use crate::{
    config::KeyboardUsbConfig,
    usb::descriptor::{CompositeReport, ViaReport},
};

static SUSPENDED: AtomicBool = AtomicBool::new(false);

// In this case, report id should be used.
// The keyboard usb device should have 3 hid instances:
// 1. Boot keyboard: 1 endpoint in
// 2. Other: Mouse + System control + Consumer control: 1 endpoint in
// 3. Via: used to communicate with via: 2 endpoints(in/out)
pub(crate) struct KeyboardUsbDevice<'d, D: Driver<'d>> {
    pub(crate) device: UsbDevice<'d, D>,
    pub(crate) keyboard_hid_writer: HidWriter<'d, D, 8>,
    pub(crate) keyboard_hid_reader: HidReader<'d, D, 1>,
    pub(crate) other_hid_writer: HidWriter<'d, D, 9>,
    pub(crate) via_hid: HidReaderWriter<'d, D, 32, 32>,
}

impl<D: Driver<'static>> KeyboardUsbDevice<'static, D> {
    pub(crate) fn new(driver: D, keyboard_config: KeyboardUsbConfig<'static>) -> Self {
        // Create embassy-usb Config
        let mut usb_config = embassy_usb::Config::new(keyboard_config.vid, keyboard_config.pid);
        usb_config.manufacturer = keyboard_config.manufacturer;
        usb_config.product = keyboard_config.product_name;
        usb_config.serial_number = keyboard_config.serial_number;
        usb_config.max_power = 450;

        // Required for windows compatibility.
        usb_config.max_packet_size_0 = 64;
        usb_config.device_class = 0xEF;
        usb_config.device_sub_class = 0x02;
        usb_config.device_protocol = 0x01;
        usb_config.composite_with_iads = true;

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
            poll_ms: 1,
            max_packet_size: 64,
        };
        static KEYBOARD_HID_STATE: StaticCell<State> = StaticCell::new();
        let keyboard_hid: HidReaderWriter<'_, D, 1, 8> = HidReaderWriter::new(
            &mut builder,
            KEYBOARD_HID_STATE.init(State::new()),
            keyboard_hid_config,
        );

        let other_hid_config = Config {
            report_descriptor: CompositeReport::desc(),
            request_handler: Some(&request_handler),
            poll_ms: 1,
            max_packet_size: 64,
        };
        static OTHER_HID_STATE: StaticCell<State> = StaticCell::new();
        let other_hid: HidWriter<'_, D, 9> = HidWriter::new(
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
        let (reader, writer) = keyboard_hid.split();
        Self {
            device: usb,
            keyboard_hid_reader: reader,
            keyboard_hid_writer: writer,
            other_hid_writer: other_hid,
            via_hid,
        }
    }
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {}", id);
        None
    }

    fn set_report(&self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {}: {}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {} to {}", id, dur);
    }

    fn get_idle_ms(&self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {}", id);
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
