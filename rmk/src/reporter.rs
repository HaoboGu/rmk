use core::{future::Future, u64};

use embassy_usb::{class::hid::HidWriter, driver::Driver};
use ssmarshal::serialize;

use crate::{
    hid::{HidError, HidWriterTrait, Report},
    keyboard::KEYBOARD_REPORT_CHANNEL,
    usb::descriptor::CompositeReportType,
};

/// Runnable trait defines `run` function for running the task
pub trait Runnable {
    /// Run function
    fn run(&mut self) -> impl Future<Output = ()>;
}

/// USB keyboard writer
/// TODO: Move to usb mod?
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

impl<'a, 'd, D: Driver<'d>> Runnable for UsbKeyboardWriter<'a, 'd, D> {
    async fn run(&mut self) {
        self.run_reporter().await;
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

pub struct DummyReporter {}

impl Runnable for DummyReporter {
    async fn run(&mut self) {
        self.run_reporter().await;
    }
}
impl HidWriterTrait for DummyReporter {
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
