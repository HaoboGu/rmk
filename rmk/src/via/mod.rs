use self::process::VialService;
use crate::hid::HidReaderWriterWrapper;
use embassy_time::Timer;
use generic_array::ArrayLength;
use typenum::NonZero;

pub(crate) mod keycode_convert;
pub(crate) mod process;
mod protocol;
mod vial;

pub(crate) async fn vial_task<
    'a,
    Hid: HidReaderWriterWrapper,
    Row: NonZero + ArrayLength,
    Col: NonZero + ArrayLength,
    NumLayers: NonZero + ArrayLength,
>(
    via_hid: &mut Hid,
    vial_service: &mut VialService<'a, Row, Col, NumLayers>,
) {
    loop {
        match vial_service.process_via_report(via_hid).await {
            Ok(_) => Timer::after_millis(1).await,
            Err(_) => Timer::after_millis(500).await,
        }
    }
}
