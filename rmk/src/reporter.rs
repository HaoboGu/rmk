use core::future::Future;

use defmt::error;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_usb::{class::hid::HidWriter, driver::Driver};
use serde::Serialize;
use ssmarshal::serialize;
use usbd_hid::descriptor::{AsInputReport, MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::{
    keyboard::KEYBOARD_REPORT_CHANNEL,
    usb::descriptor::{CompositeReportType, KeyboardReport},
    CONNECTION_STATE, REPORT_CHANNEL_SIZE,
};

#[derive(Serialize)]
pub enum Report {
    /// Normal keyboard hid report
    KeyboardReport(KeyboardReport),
    // Composite keyboard report: mouse + media(consumer) + system control
    // CompositeReport(CompositeReport),
    /// Mouse hid report
    MouseReport(MouseReport),
    /// Media keyboard report
    MediaKeyboardReport(MediaKeyboardReport),
    /// System control report
    SystemControlReport(SystemControlReport),
}

impl AsInputReport for Report {}

/// Runnable trait defines `run` function for running the task
pub trait Runnable {
    /// Run function
    async fn run(&mut self);
}

/// HidReporter trait is used for reporting HID messages to the host, via USB, BLE, etc.
pub trait HidReporter<const CHANNEL_SIZE: usize = 32> {
    /// The report type that the reporter receives from input processors.
    type ReportType: AsInputReport;

    /// Get the report receiver for the reporter.
    fn report_receiver(&self) -> Receiver<CriticalSectionRawMutex, Self::ReportType, CHANNEL_SIZE>;

    /// Run the reporter task.
    fn run_reporter(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                let report = self.report_receiver().receive().await;
                // Only send the report after the connection is established.
                if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                    self.write_report(report).await;
                }
            }
        }
    }

    /// Write report to the host
    fn write_report(&mut self, report: Self::ReportType) -> impl Future<Output = ()>;
}

/// HidListener trait is used for listening to HID messages from the host, via USB, BLE, etc.
///
/// HidListener only receives `[u8; READ_N]`, the raw HID report from the host.
/// Then processes the received message, forward to other tasks
pub trait HidListener<const READ_N: usize> {
    /// The report size from the host

    /// Read HID report from the host
    fn read_report(&mut self) -> impl Future<Output = [u8; READ_N]>;

    /// Process the received HID report.
    fn process_report(&mut self, report: [u8; READ_N]) -> impl Future<Output = ()>;

    /// Run the listener
    fn run_listener(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                let report = self.read_report().await;
                self.process_report(report).await;
            }
        }
    }
}


/// USB reporter
/// TODO: Move to usb mod?
pub struct UsbKeyboardReporter<'d, D: Driver<'d>> {
    pub(crate) keyboard_writer: HidWriter<'d, D, 8>,
    pub(crate) other_writer: HidWriter<'d, D, 9>,
}

impl<'d, D: Driver<'d>> HidReporter for UsbKeyboardReporter<'d, D> {
    type ReportType = Report;

    fn report_receiver(
        &self,
    ) -> Receiver<CriticalSectionRawMutex, Self::ReportType, REPORT_CHANNEL_SIZE> {
        KEYBOARD_REPORT_CHANNEL.receiver()
    }

    async fn write_report(&mut self, report: Self::ReportType) {
        // Write report to USB
        match report {
            Report::KeyboardReport(keyboard_report) => {
                self.keyboard_writer.write_serialize(&keyboard_report).await;
            }
            Report::MouseReport(mouse_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Mouse as u8;
                match serialize(&mut buf[1..], &mouse_report) {
                    Ok(s) => {
                        self.other_writer.write(&mut buf[0..s + 1]).await;
                    }
                    Err(_) => error!("Serialize other report error"),
                }
            }
            Report::MediaKeyboardReport(media_keyboard_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Media as u8;
                match serialize(&mut buf[1..], &media_keyboard_report) {
                    Ok(s) => {
                        self.other_writer.write(&mut buf[0..s + 1]).await;
                    }
                    Err(_) => error!("Serialize other report error"),
                }
            }
            Report::SystemControlReport(system_control_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::System as u8;
                match serialize(&mut buf[1..], &system_control_report) {
                    Ok(s) => {
                        self.other_writer.write(&mut buf[0..s + 1]).await;
                    }
                    Err(_) => error!("Serialize other report error"),
                }
            }
        };
    }
}

pub struct DummyReporter {}

impl HidReporter for DummyReporter {
    type ReportType = Report;

    fn report_receiver(
        &self,
    ) -> Receiver<CriticalSectionRawMutex, Self::ReportType, REPORT_CHANNEL_SIZE> {
        KEYBOARD_REPORT_CHANNEL.receiver()
    }

    async fn write_report(&mut self, _report: Self::ReportType) {
        // Do nothing
    }
}
