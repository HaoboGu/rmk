/// Traits and types for HID message reporting and listening.
use core::{future::Future, sync::atomic::Ordering};

use embassy_usb::class::hid::ReadError;
use embassy_usb::driver::EndpointError;
use serde::Serialize;
use usbd_hid::descriptor::{AsInputReport, MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::descriptor::KeyboardReport;
use crate::state::ConnectionState;
#[cfg(not(feature = "_no_usb"))]
use crate::usb::USB_REMOTE_WAKEUP;
use crate::CONNECTION_STATE;

#[derive(Serialize, Debug, Clone)]
pub enum Report {
    /// Normal keyboard hid report
    KeyboardReport(KeyboardReport),
    /// Mouse hid report
    MouseReport(MouseReport),
    /// Media keyboard report
    MediaKeyboardReport(MediaKeyboardReport),
    /// System control report
    SystemControlReport(SystemControlReport),
}

impl AsInputReport for Report {}

#[derive(PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HidError {
    UsbReadError(ReadError),
    UsbEndpointError(EndpointError),
    // FIXME: remove unused errors
    UsbDisabled,
    UsbPartialRead,
    BufferOverflow,
    ReportSerializeError,
    BleError,
}

/// HidWriter trait is used for reporting HID messages to the host, via USB, BLE, etc.
pub trait HidWriterTrait {
    /// The report type that the reporter receives from input processors.
    type ReportType: AsInputReport + Clone;

    /// Write report to the host, return the number of bytes written if success.
    fn write_report(&mut self, report: Self::ReportType) -> impl Future<Output = Result<usize, HidError>>;
}

/// Runnable writer
pub trait RunnableHidWriter: HidWriterTrait {
    /// Get the report to be sent to the host
    fn get_report(&mut self) -> impl Future<Output = Self::ReportType>;

    /// Run the writer task.
    fn run_writer(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                // Get report to send
                let report = self.get_report().await;
                // Only send the report after the connection is established.
                if CONNECTION_STATE.load(Ordering::Acquire) == ConnectionState::Connected.into() {
                    if let Err(e) = self.write_report(report.clone()).await {
                        error!("Failed to send report: {:?}", e);
                        #[cfg(not(feature = "_no_usb"))]
                        // If the USB endpoint is disabled, try wakeup
                        if let HidError::UsbEndpointError(EndpointError::Disabled) = e {
                            USB_REMOTE_WAKEUP.signal(());
                            // Wait 200ms for the wakeup, then send the report again
                            // Ignore the error for the second send
                            embassy_time::Timer::after_millis(200).await;
                            if let Err(e) = self.write_report(report).await {
                                error!("Failed to send report after wakeup: {:?}", e);
                            }
                        }
                    };
                }
            }
        }
    }
}

/// HidReader trait is used for listening to HID messages from the host, via USB, BLE, etc.
///
/// HidReader only receives `[u8; READ_N]`, the raw HID report from the host.
/// Then processes the received message, forward to other tasks
pub trait HidReaderTrait {
    /// Report type
    type ReportType;

    /// Read HID report from the host
    fn read_report(&mut self) -> impl Future<Output = Result<Self::ReportType, HidError>>;
}

pub struct DummyWriter {}

impl HidWriterTrait for DummyWriter {
    type ReportType = Report;

    async fn write_report(&mut self, _report: Self::ReportType) -> Result<usize, HidError> {
        Ok(0)
    }
}

impl RunnableHidWriter for DummyWriter {
    async fn run_writer(&mut self) {
        // Set CONNECTION_STATE to true to keep receiving messages from the peripheral
        CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
        loop {
            let _ = KEYBOARD_REPORT_CHANNEL.receive().await;
        }
    }

    async fn get_report(&mut self) -> Self::ReportType {
        panic!("`get_report` in Dummy writer should not be used");
    }
}

#[cfg(feature = "_nrf_ble")]
pub(crate) fn get_serial_number() -> &'static str {
    use heapless::String;
    use static_cell::StaticCell;

    static SERIAL: StaticCell<String<20>> = StaticCell::new();

    let serial = SERIAL.init_with(|| {
        let ficr = embassy_nrf::pac::FICR;
        let device_id = (u64::from(ficr.deviceid(1).read()) << 32) | u64::from(ficr.deviceid(0).read());

        let mut result = String::new();
        let _ = result.push_str("vial:f64c2b3c:");

        // Hex lookup table
        const HEX_TABLE: &[u8] = b"0123456789abcdef";
        // Add 6 hex digits to the serial number, as the serial str in BLE Device Information Service is limited to 20 bytes
        for i in 0..6 {
            let digit = (device_id >> (60 - i * 4)) & 0xF;
            // This index access is safe because digit is guaranteed to be in the range of 0-15
            let hex_char = HEX_TABLE[digit as usize] as char;
            let _ = result.push(hex_char);
        }

        result
    });

    serial.as_str()
}
