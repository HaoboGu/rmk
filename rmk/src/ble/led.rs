use crate::channel::LED_SIGNAL;
use crate::hid::{HidError, HidReaderTrait};
use crate::light::LedIndicator;

pub(crate) struct BleLedReader {}

impl HidReaderTrait for BleLedReader {
    type ReportType = LedIndicator;

    // The LED state is read from a blocking callback, so `Signal` is used to wait for the state.
    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        Ok(LED_SIGNAL.wait().await)
    }
}
