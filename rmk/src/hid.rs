/// Traits and types for HID message reporting and listening.
use core::future::Future;

use crate::{usb::descriptor::KeyboardReport, CONNECTION_STATE};
use defmt::{error, Format};
use embassy_usb::{
    class::hid::{HidReader, HidReaderWriter, HidWriter, ReadError},
    driver::{Driver, EndpointError},
};
use serde::Serialize;
use usbd_hid::descriptor::{AsInputReport, MediaKeyboardReport, MouseReport, SystemControlReport};

#[derive(Serialize)]
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
pub(crate) enum HidError {
    UsbReadError(ReadError),
    UsbEndpointError(EndpointError),
    // FIXME: remove unused errors
    UsbDisabled,
    UsbPartialRead,
    BufferOverflow,
    ReportSerializeError,
    BleDisconnected,
    BleRawError,
}

/// HidReporter trait is used for reporting HID messages to the host, via USB, BLE, etc.
pub trait HidWriterTrait {
    /// The report type that the reporter receives from input processors.
    type ReportType: AsInputReport;

    /// Get the report to be sent to the host
    fn get_report(&mut self) -> impl Future<Output = Self::ReportType>;

    /// Run the reporter task.
    fn run_reporter(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                // Get report to send
                let report = self.get_report().await;
                // Only send the report after the connection is established.
                if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                    match self.write_report(report).await {
                        Ok(_) => continue,
                        Err(e) => error!("Failed to send report: {:?}", e),
                    };
                }
            }
        }
    }

    /// Write report to the host, return the number of bytes written if success.
    fn write_report(
        &mut self,
        report: Self::ReportType,
    ) -> impl Future<Output = Result<usize, HidError>>;
}

/// HidListener trait is used for listening to HID messages from the host, via USB, BLE, etc.
///
/// HidListener only receives `[u8; READ_N]`, the raw HID report from the host.
/// Then processes the received message, forward to other tasks
pub trait HidReaderTrait {
    /// Report type
    type ReportType;

    /// Read HID report from the host
    fn read_report(&mut self) -> impl Future<Output = Result<Self::ReportType, HidError>>;
}
