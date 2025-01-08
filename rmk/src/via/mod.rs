use self::process::VialService;
use crate::{hid::HidReaderWriterWrapper, reporter::{HidListener, HidReporter}};
use embassy_time::Timer;

pub(crate) mod keycode_convert;
pub(crate) mod process;
mod protocol;
mod vial;

pub(crate) async fn vial_task<
    'a,
    RW: HidReporter + HidListener<32>,
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
