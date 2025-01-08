use core::u64;

use self::process::VialService;
use crate::{
    hid::{HidError, HidListener, HidReporter},
    usb::descriptor::ViaReport,
};
use embassy_time::Timer;
use embassy_usb::{class::hid::HidReaderWriter, driver::Driver};

pub(crate) mod keycode_convert;
pub(crate) mod process;
mod protocol;
mod vial;

pub(crate) async fn vial_task<
    'a,
    RW: HidReporter<ReportType = ViaReport> + HidListener<32, ReportType = ViaReport>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    // via_hid: &mut Hid,
    vial_service: &mut VialService<'a, RW, ROW, COL, NUM_LAYER>,
) {
    loop {
        match vial_service.process_via_report().await {
            Ok(_) => Timer::after_millis(1).await,
            Err(_) => Timer::after_millis(500).await,
        }
    }
}

pub struct UsbVialReporterListener<'d, D: Driver<'d>> {
    pub(crate) vial_reader_writer: HidReaderWriter<'d, D, 32, 32>,
}

impl<'d, D: Driver<'d>> HidReporter for UsbVialReporterListener<'d, D> {
    type ReportType = ViaReport;

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        self.vial_reader_writer.write_serialize(&report).await;
    }

    async fn run_reporter(&mut self) {
        loop {
            // Do nothing?
            embassy_time::Timer::after_secs(u64::MAX).await;
        }
    }

    fn get_report(&mut self) -> impl core::future::Future<Output = Self::ReportType> {
        todo!()
    }
}

impl<'d, D: Driver<'d>> HidListener<32> for UsbVialReporterListener<'d, D> {
    type ReportType = [u8; 32];

    async fn read_report(&mut self) -> Result<[u8; 32], HidError> {
        let mut buf = [0; 32];
        self.vial_reader_writer
            .read(&mut buf)
            .await
            .map_err(|e| HidError::UsbReadError(e))?;
        Ok(buf)
    }

    async fn process_report(&mut self, report: [u8; 32]) -> Self::ReportType {
        todo!()
    }
}
