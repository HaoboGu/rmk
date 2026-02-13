use defmt::*;
use embassy_usb::class::hid::{HidWriter, ReportId, RequestHandler, State};
use embassy_usb::control::OutResponse;

/// USB HID keyboard interface
pub struct UsbHidKeyboard<'d, D: embassy_usb::driver::Driver<'d>> {
    writer: HidWriter<'d, D, 8>,
}

impl<'d, D: embassy_usb::driver::Driver<'d>> UsbHidKeyboard<'d, D> {
    pub fn new(writer: HidWriter<'d, D, 8>) -> Self {
        Self { writer }
    }

    /// Send a keyboard HID report to the host
    /// Report format: [modifier, reserved, key1, key2, key3, key4, key5, key6]
    pub async fn send_report(&mut self, report: &[u8; 8]) -> Result<(), ()> {
        // Send via USB
        match self.writer.write(report).await {
            Ok(_) => {
                debug!("Sent keyboard report: {:?}", report);
                Ok(())
            }
            Err(_) => {
                warn!("Failed to send keyboard report");
                Err(())
            }
        }
    }

    /// Send an empty report (all keys released)
    pub async fn send_empty_report(&mut self) -> Result<(), ()> {
        let empty = [0u8; 8];
        self.send_report(&empty).await
    }
}

/// USB HID request handler (for SET_REPORT, etc.)
pub struct UsbHidRequestHandler {}

impl RequestHandler for UsbHidRequestHandler {
    fn get_report(&mut self, _id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("GET_REPORT");
        None
    }

    fn set_report(&mut self, _id: ReportId, _data: &[u8]) -> OutResponse {
        info!("SET_REPORT");
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, _id: Option<ReportId>, dur: u32) {
        info!("SET_IDLE: {}ms", dur);
    }

    fn get_idle_ms(&mut self, _id: Option<ReportId>) -> Option<u32> {
        info!("GET_IDLE");
        None
    }
}
