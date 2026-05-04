use embassy_futures::join::join4;
use embassy_futures::select::{Either, select};
use embassy_sync::signal::Signal;
#[cfg(feature = "host")]
use embassy_usb::class::hid::HidReaderWriter;
use embassy_usb::class::hid::{HidReader, HidWriter, ReportId, RequestHandler};
use embassy_usb::control::OutResponse;
use embassy_usb::driver::{Driver, EndpointError};
use embassy_usb::{Builder, Handler, UsbDevice};
use rmk_types::connection::{ConnectionType, UsbState};
use static_cell::StaticCell;
use usbd_hid::descriptor::AsInputReport;

use crate::RawMutex;
use crate::channel::USB_REPORT_CHANNEL;
use crate::config::DeviceConfig;
use crate::core_traits::Runnable;
#[cfg(feature = "steno")]
use crate::hid::StenoReport;
#[cfg(feature = "host")]
use crate::hid::ViaReport;
use crate::hid::{
    CompositeReport, CompositeReportType, HidError, HidWriterTrait, KeyboardReport, Report, run_led_reader,
};
use crate::light::UsbLedReader;
use crate::state::{current_usb_state, set_usb_state};

pub(crate) static USB_REMOTE_WAKEUP: Signal<RawMutex, ()> = Signal::new();

/// Borrowed view over the USB HID IN endpoints used by the report writer task.
///
/// `UsbTransport` owns the USB device, readers, writers, host interface, and
/// optional logger; `run` borrows those fields separately so they can run
/// concurrently without moving the whole transport into one task.
pub(crate) struct UsbKeyboardWriter<'a, 'd, D: Driver<'d>> {
    pub(crate) keyboard_writer: &'a mut HidWriter<'d, D, 8>,
    pub(crate) other_writer: &'a mut HidWriter<'d, D, 9>,
    #[cfg(feature = "steno")]
    pub(crate) steno_writer: &'a mut HidWriter<'d, D, 9>,
}

impl<'a, 'd, D: Driver<'d>> UsbKeyboardWriter<'a, 'd, D> {
    pub(crate) fn new(
        keyboard_writer: &'a mut HidWriter<'d, D, 8>,
        other_writer: &'a mut HidWriter<'d, D, 9>,
        #[cfg(feature = "steno")] steno_writer: &'a mut HidWriter<'d, D, 9>,
    ) -> Self {
        Self {
            keyboard_writer,
            other_writer,
            #[cfg(feature = "steno")]
            steno_writer,
        }
    }

    pub(crate) async fn run_writer(&mut self) -> ! {
        loop {
            let report = USB_REPORT_CHANNEL.receive().await;

            // EndpointError::Disabled never fires on non-OTG STM32/GD32
            // peripherals during suspend, so signal wakeup proactively when a
            // USB report is pending and the bus is suspended.
            if current_usb_state() == UsbState::Suspended {
                USB_REMOTE_WAKEUP.signal(());
            }

            if let Err(e) = self.write_report(&report).await {
                error!("Failed to send report: {:?}", e);

                // Belt-and-braces for OTG peripherals where Disabled is the
                // correct suspend indicator: signal wakeup, give the host a
                // moment, then retry the same report once.
                if let HidError::UsbEndpointError(EndpointError::Disabled) = e {
                    USB_REMOTE_WAKEUP.signal(());
                    embassy_time::Timer::after_millis(500).await;
                    if let Err(e) = self.write_report(&report).await {
                        error!("Failed to send report after wakeup: {:?}", e);
                    }
                }
            }
        }
    }

    async fn write_composite<R: AsInputReport>(
        &mut self,
        kind: CompositeReportType,
        report: &R,
    ) -> Result<usize, HidError> {
        let mut buf = [0u8; 9];
        buf[0] = kind as u8;
        let n = report
            .serialize(&mut buf[1..])
            .map_err(|_| HidError::ReportSerializeError)?;
        self.other_writer
            .write(&buf[0..n + 1])
            .await
            .map_err(HidError::UsbEndpointError)?;
        Ok(n)
    }
}

impl<'d, D: Driver<'d>> HidWriterTrait for UsbKeyboardWriter<'_, 'd, D> {
    type ReportType = Report;

    async fn write_report(&mut self, report: &Self::ReportType) -> Result<usize, HidError> {
        match report {
            Report::KeyboardReport(keyboard_report) => {
                let mut buf: [u8; 8] = [0; 8];
                let n: usize = keyboard_report
                    .serialize(&mut buf)
                    .map_err(|_| HidError::ReportSerializeError)?;
                self.keyboard_writer
                    .write(&buf[0..n])
                    .await
                    .map_err(HidError::UsbEndpointError)?;
                Ok(n)
            }
            Report::MouseReport(r) => self.write_composite(CompositeReportType::Mouse, r).await,
            Report::MediaKeyboardReport(r) => self.write_composite(CompositeReportType::Media, r).await,
            Report::SystemControlReport(r) => self.write_composite(CompositeReportType::System, r).await,
            #[cfg(feature = "steno")]
            Report::StenoReport(steno_report) => {
                let mut buf: [u8; 9] = [0; 9];
                let n = steno_report
                    .serialize(&mut buf)
                    .map_err(|_| HidError::ReportSerializeError)?;

                // Cap on how long a steno report write is allowed to block. The host only
                // drains the steno IN endpoint while Plover is running; without this cap the
                // writer task stalls indefinitely (and starves keyboard reports) whenever
                // Plover is absent.
                match embassy_time::with_timeout(
                    embassy_time::Duration::from_millis(5),
                    self.steno_writer.write(&buf[0..n]),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => return Err(HidError::UsbEndpointError(e)),
                    Err(_) => {} // Plover not reading; drop this report and continue
                }
                Ok(n)
            }
        }
    }
}

pub(crate) fn new_usb_builder<'d, D: Driver<'d>>(driver: D, keyboard_config: DeviceConfig<'d>) -> Builder<'d, D> {
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

    // Extra HID interfaces (usb_log, steno) overflow the 128-byte config descriptor buffer.
    #[cfg(any(feature = "usb_log", feature = "steno"))]
    const USB_BUF_SIZE: usize = 256;
    #[cfg(not(any(feature = "usb_log", feature = "steno")))]
    const USB_BUF_SIZE: usize = 128;

    static CONFIG_DESC: StaticCell<[u8; USB_BUF_SIZE]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 16]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 16]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; USB_BUF_SIZE]> = StaticCell::new();

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

/// USB transport runnable. Owns the embassy-usb device + every HID
/// reader/writer pair and runs them concurrently for the lifetime of the
/// program.
pub struct UsbTransport<D: Driver<'static>> {
    device: UsbDevice<'static, D>,
    keyboard_reader: HidReader<'static, D, 1>,
    keyboard_writer: HidWriter<'static, D, 8>,
    other_writer: HidWriter<'static, D, 9>,
    #[cfg(feature = "steno")]
    steno_writer: HidWriter<'static, D, 9>,
    #[cfg(feature = "host")]
    host_rw: HidReaderWriter<'static, D, 32, 32>,
    #[cfg(feature = "usb_log")]
    logger: Option<embassy_usb::class::cdc_acm::CdcAcmClass<'static, D>>,
}

impl<D: Driver<'static>> UsbTransport<D> {
    pub fn new(driver: D, device_config: DeviceConfig<'static>) -> Self {
        // nRF chips don't have a stable USB serial number unless one is derived
        // from the FICR. Override here so user code doesn't have to know.
        #[cfg(feature = "_nrf_ble")]
        let device_config = {
            let mut device_config = device_config;
            device_config.serial_number = crate::ble::nrf::get_serial_number();
            device_config
        };
        let mut builder: Builder<'static, D> = new_usb_builder(driver, device_config);
        // Linux's usbhid driver auto-enables power/wakeup when it probes a
        // boot-protocol keyboard, so advertise Boot/Keyboard on the primary
        // HID interface.
        let keyboard_rw = add_usb_reader_writer!(
            &mut builder,
            KeyboardReport,
            1,
            8,
            8,
            ::embassy_usb::class::hid::HidSubclass::Boot,
            ::embassy_usb::class::hid::HidBootProtocol::Keyboard
        );
        let other_writer = add_usb_writer!(&mut builder, CompositeReport, 9, 16);
        #[cfg(feature = "steno")]
        let steno_writer = add_usb_writer!(&mut builder, StenoReport, 9, 16);
        #[cfg(feature = "host")]
        let host_rw = add_usb_reader_writer!(&mut builder, ViaReport, 32, 32, 32);
        #[cfg(feature = "usb_log")]
        let logger = Some(add_usb_logger!(&mut builder));

        let (keyboard_reader, keyboard_writer) = keyboard_rw.split();
        let device = builder.build();

        Self {
            device,
            keyboard_reader,
            keyboard_writer,
            other_writer,
            #[cfg(feature = "steno")]
            steno_writer,
            #[cfg(feature = "host")]
            host_rw,
            #[cfg(feature = "usb_log")]
            logger,
        }
    }
}

impl<D: Driver<'static>> Runnable for UsbTransport<D> {
    async fn run(&mut self) -> ! {
        let Self {
            device,
            keyboard_reader,
            keyboard_writer,
            other_writer,
            #[cfg(feature = "steno")]
            steno_writer,
            #[cfg(feature = "host")]
            host_rw,
            #[cfg(feature = "usb_log")]
            logger,
        } = self;

        let usb_device_task = async {
            loop {
                device.run_until_suspend().await;
                match select(device.wait_resume(), USB_REMOTE_WAKEUP.wait()).await {
                    Either::First(_) => continue,
                    Either::Second(_) => {
                        info!("USB remote wakeup requested");
                        if device.remote_wakeup().await.is_ok() {
                            continue;
                        }
                        device.wait_resume().await;
                    }
                }
            }
        };

        let mut writer = UsbKeyboardWriter::new(
            keyboard_writer,
            other_writer,
            #[cfg(feature = "steno")]
            steno_writer,
        );
        let writer_task = writer.run_writer();

        let mut led_reader = UsbLedReader::new(keyboard_reader);
        let led_task = run_led_reader(&mut led_reader, ConnectionType::Usb);

        let host_and_extras = async {
            #[cfg(feature = "host")]
            let host_task = crate::host::usb::run_usb_host(host_rw);
            #[cfg(not(feature = "host"))]
            let host_task = core::future::pending::<()>();

            #[cfg(feature = "usb_log")]
            {
                let logger_class = logger.take().expect("UsbTransport::run called twice");
                let logger_fut = embassy_usb_logger::with_custom_style!(
                    1024,
                    log::LevelFilter::Debug,
                    logger_class,
                    |record, writer| {
                        use core::fmt::Write;
                        let ms = embassy_time::Instant::now().as_millis();
                        let _ = write!(writer, "[{:>8}ms {:5}] {}\r\n", ms, record.level(), record.args());
                    }
                );
                embassy_futures::join::join(host_task, logger_fut).await;
            }
            #[cfg(not(feature = "usb_log"))]
            host_task.await;
        };

        join4(usb_device_task, writer_task, led_task, host_and_extras).await;
        unreachable!("UsbTransport sub-tasks must run forever");
    }
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
    ($usb_builder:expr, $descriptor:ty, $n:expr) => {
        $crate::usb::add_usb_writer!($usb_builder, $descriptor, $n, 64)
    };
    // Size $max_packet to the actual report to conserve Packet Memory Area on tight parts.
    ($usb_builder:expr, $descriptor:ty, $n:expr, $max_packet:expr) => {{
        // `paste` generates per-descriptor `static`s so each writer keeps its own State/Handler.
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
            max_packet_size: $max_packet,
            hid_subclass: ::embassy_usb::class::hid::HidSubclass::No,
            hid_boot_protocol: ::embassy_usb::class::hid::HidBootProtocol::None,
        };

        let rw: ::embassy_usb::class::hid::HidWriter<_, $n> = ::embassy_usb::class::hid::HidWriter::new($usb_builder, state, hid_config);
        rw
    }};
}

macro_rules! add_usb_reader_writer {
    ($usb_builder:expr, $descriptor:ty, $read_n:expr, $write_n:expr) => {
        $crate::usb::add_usb_reader_writer!($usb_builder, $descriptor, $read_n, $write_n, 64)
    };
    // Size $max_packet to the actual report to conserve Packet Memory Area on tight parts.
    ($usb_builder:expr, $descriptor:ty, $read_n:expr, $write_n:expr, $max_packet:expr) => {
        $crate::usb::add_usb_reader_writer!(
            $usb_builder, $descriptor, $read_n, $write_n, $max_packet,
            ::embassy_usb::class::hid::HidSubclass::No,
            ::embassy_usb::class::hid::HidBootProtocol::None
        )
    };
    ($usb_builder:expr, $descriptor:ty, $read_n:expr, $write_n:expr, $max_packet:expr, $subclass:expr, $protocol:expr) => {{
        // `paste` generates per-descriptor `static`s so each reader/writer keeps its own State/Handler.
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
            max_packet_size: $max_packet,
            hid_subclass: $subclass,
            hid_boot_protocol: $protocol,
        };

        let rw: ::embassy_usb::class::hid::HidReaderWriter<_, $read_n, $write_n> = ::embassy_usb::class::hid::HidReaderWriter::new($usb_builder, state, hid_config);
        rw
    }};
}

#[cfg(feature = "usb_log")]
pub(crate) use add_usb_logger;
pub(crate) use add_usb_reader_writer;
pub(crate) use add_usb_writer;

pub(crate) struct UsbRequestHandler {}

impl RequestHandler for UsbRequestHandler {
    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {:?}", id, data);
        OutResponse::Accepted
    }
}

pub(crate) struct UsbDeviceHandler {
    /// State to restore on resume. Captured at suspend so an Enabled-but-not-yet-Configured
    /// device that suspends/resumes doesn't get incorrectly upgraded to Configured.
    pre_suspend: UsbState,
}

impl UsbDeviceHandler {
    fn new() -> Self {
        UsbDeviceHandler {
            pre_suspend: UsbState::Disabled,
        }
    }
}

impl Handler for UsbDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        if enabled {
            info!("Device enabled");
            set_usb_state(UsbState::Enabled);
        } else {
            info!("Device disabled");
            set_usb_state(UsbState::Disabled);
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
            set_usb_state(UsbState::Configured);
            info!("Device configured, it may now draw up to the configured current from Vbus.")
        } else {
            set_usb_state(UsbState::Enabled);
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }

    fn suspended(&mut self, suspended: bool) {
        // When no logging feature is enabled, `info!` expands to a no-op and
        // both arms collapse to identical empty blocks — suppress the lint.
        #[allow(clippy::if_same_then_else)]
        if suspended {
            // Snapshot the live state so resume can restore it. Skip the snapshot
            // if a stray duplicate `suspended(true)` ever fires while we're already
            // Suspended — otherwise we'd lose the original pre-suspend state.
            let live = current_usb_state();
            if live != UsbState::Suspended {
                self.pre_suspend = live;
            }
            set_usb_state(UsbState::Suspended);
            info!(
                "Device suspended, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled)."
            );
        } else {
            // Only restore from Suspended; if we're somehow not in Suspended (out-of-order
            // callbacks), don't overwrite — `configured()`/`enabled()` will resync.
            if current_usb_state() == UsbState::Suspended {
                set_usb_state(self.pre_suspend);
            }
            info!(
                "Device resumed, the Vbus current limit is 500µA (or 2.5mA for high-power devices with remote wakeup enabled)."
            );
        }
    }

    fn remote_wakeup_enabled(&mut self, enabled: bool) {
        info!("Remote wakeup enabled state: {}", enabled);
    }
}
