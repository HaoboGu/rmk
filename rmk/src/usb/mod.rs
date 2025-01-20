pub(crate) mod descriptor;

use core::sync::atomic::{AtomicU8, Ordering};
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{Config, HidWriter, ReportId, RequestHandler, State},
    control::OutResponse,
    driver::Driver,
    Builder, Handler,
};
use ssmarshal::serialize;
use static_cell::StaticCell;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::{
    config::KeyboardUsbConfig,
    hid::{HidError, HidWriterTrait, Report},
    channel::KEYBOARD_REPORT_CHANNEL,
    usb::descriptor::CompositeReportType,
    CONNECTION_STATE,
};

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
pub struct UsbKeyboardWriter<'a, 'd, D: Driver<'d>> {
    pub(crate) keyboard_writer: &'a mut HidWriter<'d, D, 8>,
    pub(crate) other_writer: &'a mut HidWriter<'d, D, 9>,
}
impl<'a, 'd, D: Driver<'d>> UsbKeyboardWriter<'a, 'd, D> {
    pub(crate) fn new(
        keyboard_writer: &'a mut HidWriter<'d, D, 8>,
        other_writer: &'a mut HidWriter<'d, D, 9>,
    ) -> Self {
        Self {
            keyboard_writer,
            other_writer,
        }
    }
}

impl<'a, 'd, D: Driver<'d>> HidWriterTrait for UsbKeyboardWriter<'a, 'd, D> {
    type ReportType = Report;

    async fn get_report(&mut self) -> Self::ReportType {
        KEYBOARD_REPORT_CHANNEL.receive().await
    }

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        // Write report to USB
        match report {
            Report::KeyboardReport(keyboard_report) => {
                self.keyboard_writer
                    .write_serialize(&keyboard_report)
                    .await
                    .map_err(|e| HidError::UsbEndpointError(e))?;
                Ok(8)
            }
            Report::MouseReport(mouse_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Mouse as u8;
                let n = serialize(&mut buf[1..], &mouse_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.other_writer
                    .write(&mut buf[0..n + 1])
                    .await
                    .map_err(|e| HidError::UsbEndpointError(e))?;
                Ok(n)
            }
            Report::MediaKeyboardReport(media_keyboard_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Media as u8;
                let n = serialize(&mut buf[1..], &media_keyboard_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.other_writer
                    .write(&mut buf[0..n + 1])
                    .await
                    .map_err(|e| HidError::UsbEndpointError(e))?;
                Ok(n)
            }
            Report::SystemControlReport(system_control_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::System as u8;
                let n = serialize(&mut buf[1..], &system_control_report)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.other_writer
                    .write(&mut buf[0..n + 1])
                    .await
                    .map_err(|e| HidError::UsbEndpointError(e))?;
                Ok(n)
            }
        }
    }
}

pub(crate) fn new_usb_builder<'d, D: Driver<'d>>(
    driver: D,
    keyboard_config: KeyboardUsbConfig<'d>,
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
macro_rules! register_usb_writer {
    ($usb_builder:expr, $descriptor:ty, $n:expr) => {{
        // Initialize hid writer
        // Current implementation requires the static STATE, so we need to use the paste crate to generate the static variable name.
        use usbd_hid::descriptor::SerializedDescriptor;
        paste::paste! {
            static [<$descriptor:snake:upper _STATE>]: ::static_cell::StaticCell<::embassy_usb::class::hid::State> = ::static_cell::StaticCell::new();
            static [<$descriptor:snake:upper _HANDLER>]: ::static_cell::StaticCell<$crate::usb::UsbRequestHandler> = ::static_cell::StaticCell::new();
        }

        let state = paste::paste! { [<$descriptor:snake:upper _STATE>].init(::embassy_usb::class::hid::State::new()) };
        let request_handler = paste::paste! { [<$descriptor:snake:upper _HANDLER>].init($crate::usb::UsbRequestHandler {}) };

        let hid_config = ::embassy_usb::class::hid::Config {
            report_descriptor: <$descriptor>::desc(),
            request_handler: Some(request_handler),
            poll_ms: 1,
            max_packet_size: 64,
        };

        let rw: ::embassy_usb::class::hid::HidWriter<_, $n> = ::embassy_usb::class::hid::HidWriter::new($usb_builder, state, hid_config);
        rw
    }};
}

#[macro_export]
macro_rules! add_usb_reader_writer {
    ($usb_builder:expr, $descriptor:ty, $read_n:expr, $write_n:expr) => {{
        // Initialize hid reader writer
        // Current implementation requires the static STATE, so we need to use the paste crate to generate the static variable name.
        use usbd_hid::descriptor::SerializedDescriptor;
        paste::paste! {
            static [<$descriptor:snake:upper _STATE>]: ::static_cell::StaticCell<::embassy_usb::class::hid::State> = ::static_cell::StaticCell::new();
            static [<$descriptor:snake:upper _HANDLER>]: ::static_cell::StaticCell<$crate::usb::UsbRequestHandler> = ::static_cell::StaticCell::new();
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

pub(crate) struct UsbRequestHandler {}

impl RequestHandler for UsbRequestHandler {
    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {:?}", id, data);
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
