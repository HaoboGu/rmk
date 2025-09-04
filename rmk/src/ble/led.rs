use rmk_types::led_indicator::LedIndicator;

use crate::channel::LED_SIGNAL;
use crate::hid::{HidError, HidReaderTrait};

pub(crate) struct BleLedReader {}

impl HidReaderTrait for BleLedReader {
    type ReportType = LedIndicator;

    // The LED state is read from a blocking callback, so `Signal` is used to wait for the state.
    async fn read_report(&mut self) -> Result<Self::ReportType, HidError> {
        Ok(LED_SIGNAL.wait().await)
    }
}
