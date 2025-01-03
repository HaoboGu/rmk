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
use heapless::Vec;

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
    buffer: [u8; SPLIT_MESSAGE_MAX_SIZE],
    n_bytes_part: usize,
}

impl<S: Read + Write> SerialSplitDriver<S> {
    pub(crate) fn new(serial: S) -> Self {
        Self {
            serial,
            buffer: [0_u8; SPLIT_MESSAGE_MAX_SIZE],
            n_bytes_part: 0,
        }
    }
}

impl<S: Read + Write> SplitReader for SerialSplitDriver<S> {
    async fn read(&mut self) -> Result<Vec<SplitMessage, 2>, SplitDriverError> {
        const SENTINEL: u8 = 0x00;
        let mut messages = Vec::new();
        while self.n_bytes_part < self.buffer.len() {
            let n_bytes = self
                .serial
                .read(&mut self.buffer[self.n_bytes_part..])
                .await
                .inspect_err(|_e| self.n_bytes_part = 0)
                .map_err(|_e| SplitDriverError::SerialError)?;
            if n_bytes == 0 {
                return Err(SplitDriverError::EmptyMessage);
            }

            self.n_bytes_part += n_bytes;
            if self.buffer[..self.n_bytes_part].contains(&SENTINEL) {
                break;
            }
        }

        let mut start_byte = 0;
        let mut end_byte = start_byte;
        let mut partial_message = false;
        while start_byte < self.n_bytes_part {
            let value = self.buffer[end_byte];
            if value == SENTINEL {
                postcard::from_bytes_cobs(&mut self.buffer[start_byte..=end_byte]).map_or_else(
                    |e| error!("Postcard deserialize split message error: {}", e),
                    |message| {
                        messages
                            .push(message)
                            .unwrap_or_else(|_m| error!("Split message vector full"));
                    },
                );
                start_byte = end_byte + 1;
                end_byte = start_byte;
                continue;
            } else if end_byte + value as usize >= self.n_bytes_part {
                partial_message = true;
                break;
            }
            // Next Zero Data Byte
            end_byte += value as usize;
        }

        if partial_message {
            // Store Partial Message for Next Read
            self.buffer.copy_within(start_byte..self.n_bytes_part, 0);
            self.n_bytes_part = self.n_bytes_part - start_byte;
        } else {
            // Reset Buffer
            self.n_bytes_part = 0;
        }

        Ok(messages)
    }
}

impl<S: Read + Write> SplitWriter for SerialSplitDriver<S> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        let bytes = postcard::to_slice_cobs(message, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            SplitDriverError::SerializeError
        })?;
        let mut remaining_bytes = bytes.len();
        while remaining_bytes > 0 {
            let sent_bytes = self
                .serial
                .write(&bytes[bytes.len() - remaining_bytes..])
                .await
                .map_err(|_e| SplitDriverError::SerialError)?;
            remaining_bytes -= sent_bytes;
        }
        Ok(bytes.len())
    }
}

/// Initialize and run the peripheral keyboard service via serial.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `serial` - serial port to send key events to central board
pub(crate) async fn initialize_serial_split_peripheral_and_run<
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
