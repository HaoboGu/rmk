use super::driver::{SplitReader, SplitWriter};
use super::SplitMessage;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::debounce::DebouncerTrait;
use crate::direct_pin::DirectPinMatrix;
use crate::keyboard::KEY_EVENT_CHANNEL;
use crate::matrix::{Matrix, MatrixTrait};
use crate::CONNECTION_STATE;
#[cfg(feature = "_nrf_ble")]
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
#[cfg(not(feature = "_nrf_ble"))]
use embedded_io_async::{Read, Write};

/// Run the split peripheral service.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `output_pins` - output gpio pins
/// * `central_addr` - (optional) central's BLE static address. This argument is enabled only for nRF BLE split now
/// * `peripheral_addr` - (optional) peripheral's BLE static address. This argument is enabled only for nRF BLE split now
/// * `serial` - (optional) serial port used to send peripheral split message. This argument is enabled only for serial split now
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
pub async fn run_rmk_split_peripheral<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_nrf_ble"))] S: Write + Read,
    const ROW: usize,
    const COL: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    #[cfg(feature = "_nrf_ble")] central_addr: [u8; 6],
    #[cfg(feature = "_nrf_ble")] peripheral_addr: [u8; 6],
    #[cfg(not(feature = "_nrf_ble"))] serial: S,
    #[cfg(feature = "_nrf_ble")] spawner: Spawner,
) {
    // Create the debouncer, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let debouncer = RapidDebouncer::<ROW, COL>::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer = RapidDebouncer::<COL, ROW>::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer = DefaultDebouncer::<COL, ROW>::new();

    // Keyboard matrix, use COL2ROW by default
    #[cfg(feature = "col2row")]
    let matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let matrix = Matrix::<_, _, _, COL, ROW>::new(input_pins, output_pins, debouncer);

    run_rmk_split_peripheral_with_matrix::<
        Matrix<In, Out, DefaultDebouncer<ROW, COL>, ROW, COL>,
        ROW,
        COL,
    >(
        matrix,
        #[cfg(feature = "_nrf_ble")]
        central_addr,
        #[cfg(feature = "_nrf_ble")]
        peripheral_addr,
        #[cfg(not(feature = "_nrf_ble"))]
        serial,
        #[cfg(feature = "_nrf_ble")]
        spawner,
    )
    .await;
}

/// Run the split peripheral service with direct pin matrix.
///
/// # Arguments
///
/// * `direct_pins` - direct gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `central_addr` - (optional) central's BLE static address. This argument is enabled only for nRF BLE split now
/// * `peripheral_addr` - (optional) peripheral's BLE static address. This argument is enabled only for nRF BLE split now
/// * `low_active`: pin active level
/// * `serial` - (optional) serial port used to send peripheral split message. This argument is enabled only for serial split now
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
pub async fn run_rmk_split_peripheral_direct_pin<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_nrf_ble"))] S: Write + Read,
    const ROW: usize,
    const COL: usize,
    const SIZE: usize,
>(
    direct_pins: [[Option<In>; COL]; ROW],
    #[cfg(feature = "_nrf_ble")] central_addr: [u8; 6],
    #[cfg(feature = "_nrf_ble")] peripheral_addr: [u8; 6],
    low_active: bool,
    #[cfg(not(feature = "_nrf_ble"))] serial: S,
    #[cfg(feature = "_nrf_ble")] spawner: Spawner,
) {
    // Create the debouncer, use COL2ROW by default
    #[cfg(feature = "rapid_debouncer")]
    let debouncer = RapidDebouncer::<COL, ROW>::new();
    #[cfg(not(feature = "rapid_debouncer"))]
    let debouncer = DefaultDebouncer::<COL, ROW>::new();

    // Keyboard matrix
    let matrix = DirectPinMatrix::<_, _, ROW, COL, SIZE>::new(direct_pins, debouncer, low_active);

    run_rmk_split_peripheral_with_matrix::<
        DirectPinMatrix<In, DefaultDebouncer<COL, ROW>, ROW, COL, SIZE>,
        ROW,
        COL,
    >(
        matrix,
        #[cfg(feature = "_nrf_ble")]
        central_addr,
        #[cfg(feature = "_nrf_ble")]
        peripheral_addr,
        #[cfg(not(feature = "_nrf_ble"))]
        serial,
        #[cfg(feature = "_nrf_ble")]
        spawner,
    )
    .await;
}

/// Run the split peripheral service.
///
/// # Arguments
///
/// * `matrix` - the matrix scanning implementation to use.
/// * `central_addr` - (optional) central's BLE static address. This argument is enabled only for nRF BLE split now
/// * `peripheral_addr` - (optional) peripheral's BLE static address. This argument is enabled only for nRF BLE split now
/// * `serial` - (optional) serial port used to send peripheral split message. This argument is enabled only for serial split now
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
pub async fn run_rmk_split_peripheral_with_matrix<
    M: MatrixTrait,
    #[cfg(not(feature = "_nrf_ble"))] S: Write + Read,
    const ROW: usize,
    const COL: usize,
>(
    matrix: M,
    #[cfg(feature = "_nrf_ble")] central_addr: [u8; 6],
    #[cfg(feature = "_nrf_ble")] peripheral_addr: [u8; 6],
    #[cfg(not(feature = "_nrf_ble"))] serial: S,
    #[cfg(feature = "_nrf_ble")] spawner: Spawner,
) {
    #[cfg(not(feature = "_nrf_ble"))]
    crate::split::serial::initialize_serial_split_peripheral_and_run::<_, S, ROW, COL>(
        matrix, serial,
    )
    .await;

    #[cfg(feature = "_nrf_ble")]
    crate::split::nrf::peripheral::initialize_nrf_ble_split_peripheral_and_run::<_, ROW, COL>(
        matrix,
        central_addr,
        peripheral_addr,
        spawner,
    )
    .await;
}

/// The split peripheral instance.
pub(crate) struct SplitPeripheral<S: SplitWriter + SplitReader> {
    split_driver: S,
}

impl<S: SplitWriter + SplitReader> SplitPeripheral<S> {
    pub(crate) fn new(split_driver: S) -> Self {
        Self { split_driver }
    }

    /// Run the peripheral keyboard service.
    ///
    /// The peripheral uses the general matrix, does scanning and send the key events through `SplitWriter`.
    /// If also receives split messages from the central through `SplitReader`.
    pub(crate) async fn run(&mut self) -> ! {
        loop {
            match select(self.split_driver.read(), KEY_EVENT_CHANNEL.receive()).await {
                embassy_futures::select::Either::First(m) => match m {
                    // Currently only handle the central state message
                    Ok(split_message) => match split_message {
                        SplitMessage::ConnectionState(state) => {
                            info!("Received connection state update: {}", state);
                            CONNECTION_STATE.store(state, core::sync::atomic::Ordering::Release);
                        }
                        _ => (),
                    },
                    Err(e) => {
                        error!("Split message read error: {:?}", e);
                    }
                },
                embassy_futures::select::Either::Second(e) => {
                    // Only send the key event if the connection is established
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        info!("Writing split message to central");
                        self.split_driver.write(&SplitMessage::Key(e)).await.ok();
                    }
                }
            }
        }
    }
}
