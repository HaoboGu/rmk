use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_io_async::Write;
use postcard::experimental::max_size::MaxSize;

#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::matrix::{Matrix, MatrixTrait};

use super::SplitMessage;

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `serial` - serial port to send key events to master board
pub async fn initialize_serial_split_slave_and_run<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    S: Write + Read,
    const ROW: usize,
    const COL: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    serial: S,
) -> ! {
    // Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let mut matrix =
        Matrix::<_, _, RapidDebouncer<ROW, COL>, ROW, COL>::new(input_pins, output_pins);
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let mut matrix =
        Matrix::<_, _, DefaultDebouncer<ROW, COL>, ROW, COL>::new(input_pins, output_pins);
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let mut matrix =
        Matrix::<_, _, RapidDebouncer<COL, ROW>, COL, ROW>::new(input_pins, output_pins);
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let mut matrix =
        Matrix::<_, _, DefaultDebouncer<COL, ROW>, COL, ROW>::new(input_pins, output_pins);

    run_slave::<_, _, ROW, COL>(&mut matrix, SerialSplitDriver::new(serial)).await
}

pub(crate) async fn run_slave<
    M: MatrixTrait,
    S: SplitWriter + SplitReader,
    const ROW: usize,
    const COL: usize,
>(
    matrix: &mut M,
    mut split_driver: S,
) -> ! {
    loop {
        matrix.scan().await;

        // Send key events to host
        for row_idx in 0..ROW {
            for col_idx in 0..COL {
                let key_state = matrix.get_key_state(row_idx, col_idx);
                if key_state.changed {
                    split_driver
                        .write(&SplitMessage::Key(
                            row_idx as u8,
                            col_idx as u8,
                            key_state.pressed,
                        ))
                        .await
                        .unwrap();
                }
            }
        }

        // 10KHZ scan rate
        embassy_time::Timer::after_micros(10).await;
    }
}
