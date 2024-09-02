use defmt::{error, info};
use embedded_io_async::{Read, Write};

use crate::split::{
    driver::{SplitMasterReceiver, SplitReader, SplitWriter},
    SplitMessage, SPLIT_MESSAGE_MAX_SIZE,
};

use super::driver::SplitDriverError;

// Receive split message from slave via serial and process it
///
/// Generic parameters:
/// - `const ROW`: row number of the slave's matrix
/// - `const COL`: column number of the slave's matrix
/// - `const ROW_OFFSET`: row offset of the slave's matrix in the whole matrix
/// - `const COL_OFFSET`: column offset of the slave's matrix in the whole matrix
/// - `S`: a serial port that implements `Read` and `Write` trait in embedded-io-async
pub async fn run_serial_slave_monitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    S: Read + Write,
>(
    receiver: S,
    id: usize,
) {
    let split_serial_driver = SerialSplitDriver::new(receiver);
    let slave =
        SplitMasterReceiver::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(split_serial_driver, id);
    info!("Running slave monitor {}", id);
    slave.run().await;
}

pub(crate) struct SerialSplitDriver<S: Read + Write> {
    serial: S,
}

impl<S: Read + Write> SerialSplitDriver<S> {
    pub fn new(serial: S) -> Self {
        Self { serial }
    }
}

impl<S: Read + Write> SplitReader for SerialSplitDriver<S> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        let n_bytes = self
            .serial
            .read(&mut buf)
            .await
            .map_err(|_e| SplitDriverError::SerialError)?;
        if n_bytes == 0 {
            return Err(SplitDriverError::EmptyMessage);
        }
        let message: SplitMessage = postcard::from_bytes(&buf).map_err(|e| {
            error!("Postcard deserialize split message error: {}", e);
            SplitDriverError::DeserializeError
        })?;
        Ok(message)
    }
}

impl<S: Read + Write> SplitWriter for SerialSplitDriver<S> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        let bytes = postcard::to_slice(message, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            SplitDriverError::SerializeError
        })?;
        self.serial
            .write(bytes)
            .await
            .map_err(|_e| SplitDriverError::SerialError)
    }
}
