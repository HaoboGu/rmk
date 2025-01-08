use core::u64;

use embassy_usb::{class::hid::HidWriter, driver::Driver};
use ssmarshal::serialize;

use crate::{
    hid::{HidError, HidReporter, Report},
    keyboard::KEYBOARD_REPORT_CHANNEL,
    usb::descriptor::CompositeReportType,
};

/// Runnable trait defines `run` function for running the task
pub trait Runnable {
    /// Run function
    async fn run(&mut self);
}

/// USB reporter
/// TODO: Move to usb mod?
pub struct UsbKeyboardReporter<'d, D: Driver<'d>> {
    pub(crate) keyboard_writer: HidWriter<'d, D, 8>,
    pub(crate) other_writer: HidWriter<'d, D, 9>,
}

impl<'d, D: Driver<'d>> Runnable for UsbKeyboardReporter<'d, D> {
    async fn run(&mut self) {
        self.run_reporter().await;
    }
}

impl<'d, D: Driver<'d>> HidReporter for UsbKeyboardReporter<'d, D> {
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
            }
            Report::MouseReport(mouse_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Mouse as u8;
                match serialize(&mut buf[1..], &mouse_report) {
                    Ok(s) => {
                        self.other_writer
                            .write(&mut buf[0..s + 1])
                            .await
                            .map_err(|e| HidError::UsbEndpointError(e))?;
                    }
                    Err(_) => return Err(HidError::SerializeError),
                }
            }
            Report::MediaKeyboardReport(media_keyboard_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::Media as u8;
                match serialize(&mut buf[1..], &media_keyboard_report) {
                    Ok(s) => {
                        self.other_writer.write(&mut buf[0..s + 1]).await;
                    }
                    Err(_) => return Err(HidError::SerializeError),
                }
            }
            Report::SystemControlReport(system_control_report) => {
                let mut buf: [u8; 9] = [0; 9];
                buf[0] = CompositeReportType::System as u8;
                match serialize(&mut buf[1..], &system_control_report) {
                    Ok(s) => {
                        self.other_writer
                            .write(&mut buf[0..s + 1])
                            .await
                            .map_err(|e| HidError::UsbEndpointError(e))?;
                    }
                    Err(_) => return Err(HidError::SerializeError),
                }
            }
        };
    }
}

pub struct DummyReporter {}

impl Runnable for DummyReporter {
    async fn run(&mut self) {
        self.run_reporter().await;
    }
}
impl HidReporter for DummyReporter {
    type ReportType = Report;

    async fn write_report(&mut self, _report: Self::ReportType) -> Result<usize, HidError> {
        loop {
            // Wait forever
            embassy_time::Timer::after_secs(u64::MAX).await
        }
    }

    async fn get_report(&mut self) -> Self::ReportType {
        loop {
            // Wait forever
            embassy_time::Timer::after_secs(u64::MAX).await
        }
    }
}
