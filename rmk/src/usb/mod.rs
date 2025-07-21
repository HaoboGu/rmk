use core::sync::atomic::Ordering;

use embassy_sync::signal::Signal;
use embassy_usb::class::hid::{HidWriter, ReportId, RequestHandler};
use embassy_usb::control::OutResponse;
use embassy_usb::driver::Driver;
use embassy_usb::{Builder, Handler};
use ssmarshal::serialize;
use static_cell::StaticCell;

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::config::KeyboardUsbConfig;
use crate::descriptor::CompositeReportType;
use crate::hid::{HidError, HidWriterTrait, Report, RunnableHidWriter};
use crate::state::ConnectionState;
use crate::{RawMutex, CONNECTION_STATE};

pub(crate) static USB_REMOTE_WAKEUP: Signal<RawMutex, ()> = Signal::new();

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

pub(crate) struct UsbKeyboardWriter<'a, 'd, D: Driver<'d>> {
    pub(crate) keyboard_writer: &'a mut HidWriter<'d, D, 8>,
    pub(crate) other_writer: &'a mut HidWriter<'d, D, 9>,
}
impl<'a, 'd, D: Driver<'d>> UsbKeyboardWriter<'a, 'd, D> {
    pub(crate) fn new(keyboard_writer: &'a mut HidWriter<'d, D, 8>, other_writer: &'a mut HidWriter<'d, D, 9>) -> Self {
        Self {
            keyboard_writer,
            other_writer,
        }
    }
}

impl<'d, D: Driver<'d>> RunnableHidWriter for UsbKeyboardWriter<'_, 'd, D> {
    async fn get_report(&mut self) -> Self::ReportType {
        KEYBOARD_REPORT_CHANNEL.receive().await
    }
}

impl<'d, D: Driver<'d>> HidWriterTrait for UsbKeyboardWriter<'_, 'd, D> {
    type ReportType = Report;

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        // Write report to USB
        match report {
            Report::KeyboardReport(keyboard_report) => {
                self.keyboard_writer
                    .write_serialize(&keyboard_report)
                    .await
                    .map_err(HidError::UsbEndpointError)?;
                Ok(8)
            }
            Report::MouseReport(mouse_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Mouse as u8;
                let n = serialize(&mut buf[1..], &mouse_report).map_err(|_| HidError::ReportSerializeError)?;
                self.other_writer
                    .write(&buf[0..n + 1])
                    .await
                    .map_err(HidError::UsbEndpointError)?;
                Ok(n)
            }
            Report::MediaKeyboardReport(media_keyboard_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Media as u8;
                let n = serialize(&mut buf[1..], &media_keyboard_report).map_err(|_| HidError::ReportSerializeError)?;
                self.other_writer
                    .write(&buf[0..n + 1])
                    .await
                    .map_err(HidError::UsbEndpointError)?;
                Ok(n)
            }
            Report::SystemControlReport(system_control_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::System as u8;
                let n = serialize(&mut buf[1..], &system_control_report).map_err(|_| HidError::ReportSerializeError)?;
                self.other_writer
                    .write(&buf[0..n + 1])
                    .await
                    .map_err(HidError::UsbEndpointError)?;
                Ok(n)
            }
        }
    }
}

pub(crate) fn new_usb_builder<'d, D: Driver<'d>>(driver: D, keyboard_config: KeyboardUsbConfig<'d>) -> Builder<'d, D> {
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

    #[cfg(feature = "usb_log")]
    const USB_BUF_SIZE: usize = 256;
    #[cfg(not(feature = "usb_log"))]
    const USB_BUF_SIZE: usize = 128;

    // Create embassy-usb DeviceBuilder using the driver and config.
    static CONFIG_DESC: StaticCell<[u8; USB_BUF_SIZE]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 16]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 16]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; USB_BUF_SIZE]> = StaticCell::new();

    // UsbDevice builder
    let mut builder = Builder::new(
        driver,
        usb_config,
        &mut CONFIG_DESC.init([0; USB_BUF_SIZE])[..],
        &mut BOS_DESC.init([0; 16])[..],
        &mut MSOS_DESC.init([0; 16])[..],
        &mut CONTROL_BUF.init([0; USB_BUF_SIZE])[..],
    );

    static device_handler: StaticCell<UsbDeviceHandler> = StaticCell::new();
    builder.handler(device_handler.init(UsbDeviceHandler::new()));

    builder
}

#[cfg(feature = "usb_log")]
macro_rules! add_usb_logger {
    ($usb_builder:expr) => {{
        use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
        use static_cell::StaticCell;

        // The usb logger can be only initialized once, so just use a fixed name for the state
        static LOGGER_STATE: StaticCell<State> = StaticCell::new();
        let state = LOGGER_STATE.init(State::new());
        CdcAcmClass::new($usb_builder, state, 64)
    }};
}

macro_rules! add_usb_writer {
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

#[cfg(feature = "usb_log")]
pub(crate) use add_usb_logger;
pub(crate) use {add_usb_reader_writer, add_usb_writer};

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

pub(crate) static USB_ENABLED: Signal<crate::RawMutex, ()> = Signal::new();
pub(crate) static USB_SUSPENDED: Signal<crate::RawMutex, ()> = Signal::new();

impl Handler for UsbDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        if enabled {
            info!("Device enabled");
            USB_ENABLED.signal(());
        } else {
            info!("Device disabled");
            if USB_ENABLED.signaled() {
                USB_ENABLED.reset();
            }
        }
    }

    fn reset(&mut self) {
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        if configured {
            CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
            USB_ENABLED.signal(());
            info!("Device configured, it may now draw up to the configured current from Vbus.")
        } else {
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }

    fn suspended(&mut self, suspended: bool) {
        if suspended {
            info!("Device suspended, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled).");
            USB_SUSPENDED.signal(());
        } else {
            info!("Device resumed, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled).");
            USB_SUSPENDED.reset();
        }
    }

    fn remote_wakeup_enabled(&mut self, enabled: bool) {
        info!("Remote wakeup enabled state: {}", enabled);
    }
}
