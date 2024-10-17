#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::keyboard::key_event_channel;
use crate::matrix::{Matrix, MatrixTrait};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
#[cfg(feature = "_nrf_ble")]
use {
    crate::ble::nrf::softdevice_task,
    core::mem,
    embassy_executor::Spawner,
    nrf_softdevice::ble::gatt_server::set_sys_attrs,
    nrf_softdevice::ble::peripheral::{advertise_connectable, ConnectableAdvertisement},
    nrf_softdevice::ble::{set_address, Address, AddressType},
    nrf_softdevice::{raw, Config, Softdevice},
};

use super::{
    driver::{SplitReader, SplitWriter},
    SplitMessage,
};

#[cfg(not(feature = "_nrf_ble"))]
use {
    super::serial::SerialSplitDriver,
    embedded_io_async::{Read, Write},
};

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
    #[cfg(not(feature = "_nrf_ble"))]
    initialize_serial_split_peripheral_and_run::<In, Out, S, ROW, COL>(
        input_pins,
        output_pins,
        serial,
    )
    .await;

    #[cfg(feature = "_nrf_ble")]
    initialize_nrf_ble_split_peripheral_and_run::<In, Out, ROW, COL>(
        input_pins,
        output_pins,
        central_addr,
        peripheral_addr,
        spawner,
    )
    .await;
}

/// Initialize and run the nRF peripheral keyboard service via BLE.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `spwaner` - embassy task spwaner, used to spawn nrf_softdevice background task
#[cfg(feature = "_nrf_ble")]
pub(crate) async fn initialize_nrf_ble_split_peripheral_and_run<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    central_addr: [u8; 6],
    peripheral_addr: [u8; 6],
    spawner: Spawner,
) -> ! {
    use defmt::info;
    use embassy_futures::select::select3;
    use nrf_softdevice::ble::gatt_server;

    use crate::split::nrf::peripheral::{
        BleSplitPeripheralDriver, BleSplitPeripheralServer, BleSplitPeripheralServerEvent,
        SplitBleServiceEvent,
    };

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

    let ble_config = Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
            // External osc
            // source: raw::NRF_CLOCK_LF_SRC_XTAL as u8,
            // rc_ctiv: 0,
            // rc_temp_ctiv: 0,
            // accuracy: raw::NRF_CLOCK_LF_ACCURACY_20_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 4,
            central_role_count: 4,
            central_sec_count: 4,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: "rmk_peripheral_board".as_ptr() as _,
            current_len: "rmk_peripheral_board".len() as u16,
            max_len: "rmk_peripheral_board".len() as u16,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };

    let sd = Softdevice::enable(&ble_config);
    set_address(
        sd,
        &Address::new(AddressType::RandomStatic, peripheral_addr),
    );

    {
        // Use the immutable ref of `Softdevice` to run the softdevice_task
        // The mumtable ref is used for configuring Flash and BleServer
        let sdv = unsafe { nrf_softdevice::Softdevice::steal() };
        defmt::unwrap!(spawner.spawn(softdevice_task(sdv)))
    };

    let server = defmt::unwrap!(BleSplitPeripheralServer::new(sd));

    loop {
        let advertisement = ConnectableAdvertisement::NonscannableDirected {
            peer: Address::new(AddressType::RandomStatic, central_addr),
        };
        let conn = match advertise_connectable(sd, advertisement, &Default::default()).await {
            Ok(conn) => conn,
            Err(e) => {
                defmt::error!("Split peripheral advertise error: {}", e);
                continue;
            }
        };

        // Set sys attr of peripheral
        set_sys_attrs(&conn, None).unwrap();

        let server_fut = gatt_server::run(&conn, &server, |event| match event {
            BleSplitPeripheralServerEvent::Service(split_event) => match split_event {
                SplitBleServiceEvent::MessageToCentralCccdWrite { notifications } => {
                    info!("Split value CCCD updated: {}", notifications)
                }
                SplitBleServiceEvent::MessageToPeripheralWrite(message) => {
                    // TODO: Handle message from central to peripheral
                    info!("Message from central: {:?}", message);
                }
            },
        });

        let mut peripheral = SplitPeripheral::new(BleSplitPeripheralDriver::new(&server, &conn));
        let peripheral_fut = peripheral.run();
        let matrix_fut = matrix.scan();
        select3(matrix_fut, server_fut, peripheral_fut).await;
    }
}

/// Initialize and run the peripheral keyboard service via serial.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `serial` - serial port to send key events to central board
#[cfg(not(feature = "_nrf_ble"))]
pub(crate) async fn initialize_serial_split_peripheral_and_run<
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
    use embassy_futures::select::select;
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

    let mut peripheral = SplitPeripheral::new(SerialSplitDriver::new(serial));
    loop {
        select(matrix.scan(), peripheral.run()).await;
    }
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
            let e = key_event_channel.receive().await;

            self.split_driver.write(&SplitMessage::Key(e)).await.ok();

            // 10KHZ scan rate
            embassy_time::Timer::after_micros(10).await;
        }
    }
}
