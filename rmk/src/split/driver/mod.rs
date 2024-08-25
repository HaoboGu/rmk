use super::SplitMessage;

// TODO: feature gate
pub(crate) mod nrf_ble;
pub(crate) mod serial;

#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum SplitDriverError {
    SerialError,
    EmptyMessage,
    DeserializeError,
    SerializeError,
    BleError(u8),
}

pub(crate) trait SplitReader {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError>;
}
pub(crate) trait SplitWriter {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError>;
}
