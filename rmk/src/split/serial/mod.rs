use defmt::{error, info};
use embedded_io_async::{Read, Write};

use crate::{
    matrix::MatrixTrait,
    split::{
        driver::{PeripheralMatrixMonitor, SplitReader, SplitWriter},
        peripheral::SplitPeripheral,
        SplitMessage, SPLIT_MESSAGE_MAX_SIZE,
    },
};

use super::driver::SplitDriverError;

// Receive split message from peripheral via serial and process it
///
/// Generic parameters:
/// - `const ROW`: row number of the peripheral's matrix
/// - `const COL`: column number of the peripheral's matrix
/// - `const ROW_OFFSET`: row offset of the peripheral's matrix in the whole matrix
/// - `const COL_OFFSET`: column offset of the peripheral's matrix in the whole matrix
/// - `S`: a serial port that implements `Read` and `Write` trait in embedded-io-async
pub(crate) async fn run_serial_peripheral_monitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    S: Read + Write,
>(
    id: usize,
    receiver: S,
) {
    let split_serial_driver: SerialSplitDriver<S> = SerialSplitDriver::new(receiver);
    let peripheral = PeripheralMatrixMonitor::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(
        split_serial_driver,
        id,
    );
    info!("Running peripheral monitor {}", id);
    peripheral.run().await;
}

/// Serial driver for BOTH split central and peripheral
pub(crate) struct SerialSplitDriver<S: Read + Write> {
    serial: S,
}

impl<S: Read + Write> SerialSplitDriver<S> {
    pub(crate) fn new(serial: S) -> Self {
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

/// Initialize and run the peripheral keyboard service via serial.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `serial` - serial port to send key events to central board
pub async fn initialize_serial_split_peripheral_and_run<
    M: MatrixTrait,
    S: Write + Read,
    const ROW: usize,
    const COL: usize,
>(
    mut matrix: M,
    serial: S,
) -> ! {
    use embassy_futures::select::select;

    let mut peripheral = SplitPeripheral::new(SerialSplitDriver::new(serial));
    loop {
        select(matrix.run(), peripheral.run()).await;
    }
}
