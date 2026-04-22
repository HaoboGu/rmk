//! USB HID transport for Vial (fixed 32-byte reports).
//!
//! Wraps an `embassy_usb::class::hid::HidReaderWriter<32, 32>` allocated on
//! the embassy-usb builder at setup time. Implements the crate's existing
//! `HidReaderTrait` / `HidWriterTrait` with `ReportType = ViaReport`, the
//! same pattern used for keyboard/LED reports.

use embassy_usb::class::hid::HidReaderWriter;
use embassy_usb::driver::Driver;
use usbd_hid::descriptor::AsInputReport as _;

use crate::descriptor::ViaReport;
use crate::hid::{HidError, HidReaderTrait, HidWriterTrait};

const USB_HID_FRAME: usize = 32;

pub(crate) struct UsbVialReaderWriter<'d, D: Driver<'d>> {
    inner: HidReaderWriter<'d, D, USB_HID_FRAME, USB_HID_FRAME>,
}

impl<'d, D: Driver<'d>> UsbVialReaderWriter<'d, D> {
    pub(crate) fn new(inner: HidReaderWriter<'d, D, USB_HID_FRAME, USB_HID_FRAME>) -> Self {
        Self { inner }
    }
}

impl<'d, D: Driver<'d>> HidReaderTrait for UsbVialReaderWriter<'d, D> {
    type ReportType = ViaReport;

    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let mut report = ViaReport {
            input_data: [0u8; USB_HID_FRAME],
            output_data: [0u8; USB_HID_FRAME],
        };
        self.inner
            .read(&mut report.output_data)
            .await
            .map_err(HidError::UsbReadError)?;
        Ok(report)
    }
}

impl<'d, D: Driver<'d>> HidWriterTrait for UsbVialReaderWriter<'d, D> {
    type ReportType = ViaReport;

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        let mut buf = [0u8; USB_HID_FRAME];
        let n = report
            .serialize(&mut buf)
            .map_err(|_| HidError::ReportSerializeError)?;
        self.inner.write(&buf[..n]).await.map_err(HidError::UsbEndpointError)?;
        Ok(n)
    }
}
