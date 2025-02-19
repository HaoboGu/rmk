use crate::{
    channel::LED_SIGNAL,
    hid::{HidError, HidReaderTrait},
    light::LedIndicator,
};

pub(crate) struct BleLedReader {}

impl HidReaderTrait for BleLedReader {
    type ReportType = LedIndicator;

    // ESP BLE provides only a blocking callback function for reading data.
    // So we use a channel to do async read
    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        let signal = LED_SIGNAL.wait().await;
        LED_SIGNAL.reset();
        Ok(signal)
    }
}
