pub(crate) mod descriptor;

use core::sync::atomic::{AtomicBool, Ordering};
use defmt::info;
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{Config, HidReaderWriter, HidWriter, ReportId, RequestHandler, State},
    control::OutResponse,
    driver::Driver,
    Builder, Handler, UsbDevice,
};
use rmk_config::KeyboardUsbConfig;
use static_cell::StaticCell;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

use crate::{
    hid::{UsbHidReader, UsbHidReaderWriter, UsbHidWriter},
    usb::descriptor::{CompositeReport, ViaReport},
};

pub(crate) static USB_SUSPENDED: AtomicBool = AtomicBool::new(false);
pub(crate) static USB_CONFIGURED: AtomicBool = AtomicBool::new(false);
pub(crate) static USB_DEVICE_ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) async fn wait_for_usb_suspend() {
    loop {
        let suspended = USB_SUSPENDED.load(core::sync::atomic::Ordering::Acquire);
        let enabled = USB_DEVICE_ENABLED.load(core::sync::atomic::Ordering::Acquire);
        if suspended || (!enabled) {
            break;
        }
        // Check usb suspended state every 10ms
        Timer::after_millis(10).await
    }
}

pub(crate) async fn wait_for_usb_configured() {
    loop {
        let suspended = USB_CONFIGURED.load(core::sync::atomic::Ordering::Acquire);
        if suspended {
            break;
        }
        // Check usb configured state every 10ms
        Timer::after_millis(10).await
    }
}

// In this case, report id should be used.
// The keyboard usb device should have 3 hid instances:
// 1. Boot keyboard: 1 endpoint in
// 2. Other: Mouse + System control + Consumer control: 1 endpoint in
// 3. Via: used to communicate with via: 2 endpoints(in/out)
pub(crate) struct KeyboardUsbDevice<'d, D: Driver<'d>> {
    pub(crate) device: UsbDevice<'d, D>,
    pub(crate) keyboard_hid_writer: UsbHidWriter<'d, D, 8>,
    pub(crate) keyboard_hid_reader: UsbHidReader<'d, D, 1>,
    pub(crate) other_hid_writer: UsbHidWriter<'d, D, 9>,
    pub(crate) via_hid: UsbHidReaderWriter<'d, D, 32, 32>,
}

impl<D: Driver<'static>> KeyboardUsbDevice<'static, D> {
    pub(crate) fn new(driver: D, keyboard_config: KeyboardUsbConfig<'static>) -> Self {
        // Create embassy-usb Config
        let mut usb_config = embassy_usb::Config::new(keyboard_config.vid, keyboard_config.pid);
        usb_config.manufacturer = Some(keyboard_config.manufacturer);
        usb_config.product = Some(keyboard_config.product_name);
        usb_config.serial_number = Some(keyboard_config.serial_number);
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

        static device_handler: StaticCell<UsbDeviceHandler> = StaticCell::new();
        builder.handler(device_handler.init(UsbDeviceHandler::new()));

        // Create classes on the builder.
        static request_handler: UsbRequestHandler = UsbRequestHandler {};

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
            keyboard_hid_reader: UsbHidReader::new(reader),
            keyboard_hid_writer: UsbHidWriter::new(writer),
            other_hid_writer: UsbHidWriter::new(other_hid),
            via_hid: UsbHidReaderWriter::new(via_hid),
        }
    }
}

struct UsbRequestHandler {}

impl RequestHandler for UsbRequestHandler {
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

struct UsbDeviceHandler {}

impl UsbDeviceHandler {
    fn new() -> Self {
        UsbDeviceHandler {}
    }
}

impl Handler for UsbDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        USB_CONFIGURED.store(false, Ordering::Relaxed);
        USB_SUSPENDED.store(false, Ordering::Release);
        if enabled {
            USB_DEVICE_ENABLED.store(true, Ordering::Release);
            info!("Device enabled");
        } else {
            USB_DEVICE_ENABLED.store(false, Ordering::Release);
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        USB_CONFIGURED.store(false, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        USB_CONFIGURED.store(false, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        USB_CONFIGURED.store(configured, Ordering::Relaxed);
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
            USB_SUSPENDED.store(true, Ordering::Release);
        } else {
            USB_SUSPENDED.store(false, Ordering::Release);
            if USB_CONFIGURED.load(Ordering::Relaxed) {
                info!(
                    "Device resumed, it may now draw up to the configured current limit from Vbus"
                );
            } else {
                info!("Device resumed, the Vbus current limit is 100mA");
            }
        }
    }
}
