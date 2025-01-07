pub(crate) mod descriptor;

use core::sync::atomic::{AtomicU8, Ordering};
use defmt::info;
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{Config, HidReader, HidReaderWriter, HidWriter, ReportId, RequestHandler, State},
    control::OutResponse,
    driver::Driver,
    Builder, Handler,
};
use static_cell::StaticCell;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::{config::KeyboardUsbConfig, CONNECTION_STATE};

pub(crate) static USB_STATE: AtomicU8 = AtomicU8::new(UsbState::Disabled as u8);

/// USB state
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum UsbState {
    // Disconnected
    Disabled = 0x0,
    // Connected, but NOT ready
    Enabled = 0x1,
    // Connected, ready to use
    Configured = 0x2,
}

impl From<u8> for UsbState {
    fn from(state: u8) -> Self {
        match state {
            0 => UsbState::Disabled,
            1 => UsbState::Enabled,
            2 => UsbState::Configured,
            _ => UsbState::Disabled,
        }
    }
}

pub(crate) async fn wait_for_usb_suspend() {
    loop {
        // Check usb suspend state every 500ms
        Timer::after_millis(500).await;
        let usb_state: UsbState = USB_STATE.load(Ordering::Acquire).into();
        if usb_state != UsbState::Configured {
            break;
        }
    }
}

/// Wait for USB connected(but USB might not be configured yet)
pub(crate) async fn wait_for_usb_enabled() {
    loop {
        // Check usb enable state every 500ms
        Timer::after_millis(500).await;

        let usb_state: UsbState = USB_STATE.load(Ordering::Acquire).into();
        if usb_state == UsbState::Enabled {
            break;
        }
    }
}

pub(crate) fn new_usb_builder<'d, D: Driver<'d>>(
    driver: D,
    keyboard_config: KeyboardUsbConfig<'static>,
) -> Builder<'d, D> {
    // Create embassy-usb Config
    let mut usb_config = embassy_usb::Config::new(keyboard_config.vid, keyboard_config.pid);
    usb_config.manufacturer = Some(keyboard_config.manufacturer);
    usb_config.product = Some(keyboard_config.product_name);
    usb_config.serial_number = Some(keyboard_config.serial_number);
    usb_config.max_power = 450;
    usb_config.supports_remote_wakeup = true;

    // Required for windows compatibility.
    usb_config.max_packet_size_0 = 64;
    usb_config.device_class = 0xEF;
    usb_config.device_sub_class = 0x02;
    usb_config.device_protocol = 0x01;
    usb_config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 128]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();

    // UsbDevice builder
    let mut builder = Builder::new(
        driver,
        usb_config,
        &mut CONFIG_DESC.init([0; 256])[..],
        &mut BOS_DESC.init([0; 256])[..],
        &mut MSOS_DESC.init([0; 128])[..],
        &mut CONTROL_BUF.init([0; 128])[..],
    );

    static device_handler: StaticCell<UsbDeviceHandler> = StaticCell::new();
    builder.handler(device_handler.init(UsbDeviceHandler::new()));

    builder
}

// In this case, report id should be used.
// The keyboard usb device should have 3 hid instances:
// 1. Boot keyboard: 1 endpoint in
// 2. Other: Mouse + System control + Consumer control: 1 endpoint in
// 3. Via: used to communicate with via: 2 endpoints(in/out)
// pub(crate) struct KeyboardUsbDevice<'d, D: Driver<'d>, R: UsbReporterTrait<'d, D>> {
//     pub(crate) device: UsbDevice<'d, D>,
//     pub(crate) usb_reporter: R,
//     // pub(crate) keyboard_hid_writer: UsbHidWriter<'d, D, 8>,
//     pub(crate) keyboard_hid_reader: UsbHidReader<'d, D, 1>,
//     // pub(crate) other_hid_writer: UsbHidWriter<'d, D, 9>,
//     pub(crate) via_hid: UsbHidReaderWriter<'d, D, 32, 32>,
// }

// impl<D: Driver<'static>, R: UsbReporterTrait<'static, D>> KeyboardUsbDevice<'static, D, R> {
//     pub(crate) fn new(driver: D, keyboard_config: KeyboardUsbConfig<'static>) -> Self {
//         let mut builder = new_usb_builder(driver, keyboard_config);

//         // Create classes on the builder.
//         let keyboard_hid = build_usb_reader_writer::<D, KeyboardReport, 1, 8>(&mut builder);
//         let other_hid = build_usb_reader_writer::<D, CompositeReport, 0, 9>(&mut builder);
//         let via_hid = build_usb_reader_writer::<D, ViaReport, 32, 32>(&mut builder);

//         let rp = R::new(&mut builder);

//         // Build usb device
//         let usb = builder.build();
//         let (reader, writer) = keyboard_hid.split();
//         Self {
//             device: usb,
//             keyboard_hid_reader: UsbHidReader::new(reader),
//             usb_reporter: rp,
//             // keyboard_hid_writer: UsbHidWriter::new(writer),
//             // other_hid_writer: UsbHidWriter::new(other_hid.split().1),
//             via_hid: UsbHidReaderWriter::new(via_hid),
//         }
//     }
// }

pub(crate) fn register_usb_writer<D: Driver<'static>, SD: SerializedDescriptor, const N: usize>(
    usb_builder: &mut Builder<'static, D>,
) -> HidWriter<'static, D, N> {
    // Initialize hid interfaces
    static request_handler: StaticCell<UsbRequestHandler> = StaticCell::new();
    let hid_config = Config {
        report_descriptor: SD::desc(),
        request_handler: Some(request_handler.init(UsbRequestHandler {})),
        poll_ms: 1,
        max_packet_size: 64,
    };
    static STATE: StaticCell<State> = StaticCell::new();
    HidWriter::new(usb_builder, STATE.init(State::new()), hid_config)
}

#[macro_export]
macro_rules! add_usb_reader_writer {
    ($usb_builder:expr, $descriptor:ty, $read_n:expr, $write_n:expr) => {{
        // 静态变量名基于类型名称生成
        use usbd_hid::descriptor::SerializedDescriptor;
        paste::paste! {
            static [<$descriptor:snake:upper _STATE>]: StaticCell<::embassy_usb::class::hid::State> = StaticCell::new();
            static [<$descriptor:snake:upper _HANDLER>]: StaticCell<$crate::usb::UsbRequestHandler> = StaticCell::new();
        }

        let state = paste::paste! { [<$descriptor:snake:upper _STATE>].init(::embassy_usb::class::hid::State::new()) };
        let request_handler = paste::paste! { [<$descriptor:snake:upper _HANDLER>].init($crate::usb::UsbRequestHandler {}) };

        let hid_config = ::embassy_usb::class::hid::Config {
            report_descriptor: <$descriptor>::desc(),
            request_handler: Some(request_handler),
            poll_ms: 1,
            max_packet_size: 64,
        };

        let rw: ::embassy_usb::class::hid::HidReaderWriter<_, $read_n, $write_n> = ::embassy_usb::class::hid::HidReaderWriter::new($usb_builder, state, hid_config);
        rw
    }};
}

pub(crate) fn register_usb_reader_writer<
    D: Driver<'static>,
    SD: SerializedDescriptor,
    const READ_N: usize,
    const WRITE_N: usize,
>(
    usb_builder: &mut Builder<'static, D>,
) -> HidReaderWriter<'static, D, READ_N, WRITE_N> {
    static request_handler: StaticCell<UsbRequestHandler> = StaticCell::new();
    let hid_config = Config {
        report_descriptor: SD::desc(),
        request_handler: Some(request_handler.init(UsbRequestHandler {})),
        poll_ms: 1,
        max_packet_size: 64,
    };
    static STATE: StaticCell<State> = StaticCell::new();
    HidReaderWriter::new(usb_builder, STATE.init(State::new()), hid_config)
}

pub(crate) struct UsbRequestHandler {}

impl RequestHandler for UsbRequestHandler {
    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {}: {}", id, data);
        OutResponse::Accepted
    }
}

pub(crate) struct UsbDeviceHandler {}

impl UsbDeviceHandler {
    fn new() -> Self {
        UsbDeviceHandler {}
    }
}

impl Handler for UsbDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        if enabled {
            USB_STATE.store(UsbState::Enabled as u8, Ordering::Relaxed);
            info!("Device enabled");
        } else {
            USB_STATE.store(UsbState::Disabled as u8, Ordering::Relaxed);
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        USB_STATE.store(UsbState::Enabled as u8, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        USB_STATE.store(UsbState::Enabled as u8, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        if configured {
            USB_STATE.store(UsbState::Configured as u8, Ordering::Relaxed);
            CONNECTION_STATE.store(true, Ordering::Release);
            info!("Device configured, it may now draw up to the configured current from Vbus.")
        } else {
            USB_STATE.store(UsbState::Enabled as u8, Ordering::Relaxed);
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }

    fn suspended(&mut self, suspended: bool) {
        USB_STATE.store(UsbState::Enabled as u8, Ordering::Release);
        if suspended {
            info!("Device suspended, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled).");
        } else {
            info!("Device resumed, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled).");
        }
    }
}
