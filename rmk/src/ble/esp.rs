pub(crate) mod server;

use self::server::{BleServer, VialReaderWriter};
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::keyboard::keyboard_report_channel;
use crate::matrix::{Matrix, MatrixTrait};
use crate::storage::nor_flash::esp_partition::{Partition, PartitionType};
use crate::storage::Storage;
use crate::via::process::VialService;
use crate::via::vial_task;
use crate::KEYBOARD_STATE;
use crate::{
    action::KeyAction, ble::ble_communication_task, config::RmkConfig, keyboard::Keyboard,
    keyboard_task, keymap::KeyMap,
};
use core::cell::RefCell;
use defmt::{debug, info, warn};
use embassy_futures::select::{select, select4};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_storage_async::nor_flash::ReadNorFlash;
use esp_idf_svc::hal::task::block_on;
use futures::pin_mut;
use rmk_config::StorageConfig;

/// Initialize and run the BLE keyboard service, with given keyboard usb config.
/// Can only be used on nrf52 series microcontrollers with `nrf-softdevice` crate.
/// This function never returns.
///
/// # Arguments
///
/// * `keymap` - default keymap definition
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `spwaner` - embassy task spwaner, used to spawn nrf_softdevice background task
pub(crate) async fn initialize_esp_ble_keyboard_with_config_and_run<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    default_keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    let f = Partition::new(PartitionType::Custom, Some(c"rmk"));
    let num_sectors = (f.capacity() / Partition::SECTOR_SIZE) as u8;
    let mut storage = Storage::new(
        f,
        &default_keymap,
        StorageConfig {
            start_addr: 0,
            num_sectors,
        },
    )
    .await;

    let keymap = RefCell::new(
        KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(default_keymap, Some(&mut storage)).await,
    );

    let keyboard_report_sender = keyboard_report_channel.sender();
    let keyboard_report_receiver = keyboard_report_channel.receiver();

    // Keyboard matrix
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

    let mut keyboard = Keyboard::new(&keymap, &keyboard_report_sender);
    // esp32c3 doesn't have USB device, so there is no usb here
    // TODO: add usb service for other chips of esp32 which have USB device

    static via_output: Channel<CriticalSectionRawMutex, [u8; 32], 2> = Channel::new();
    let mut vial_service = VialService::new(&keymap, keyboard_config.vial_config);
    loop {
        KEYBOARD_STATE.store(false, core::sync::atomic::Ordering::Release);
        info!("Advertising..");
        let mut ble_server = BleServer::new(keyboard_config.usb_config);
        ble_server.output_keyboard.lock().on_write(|args| {
            let data: &[u8] = args.recv_data();
            debug!("output_keyboard {}, {}", data.len(), data[0]);
        });

        info!("Waitting for connection..");
        ble_server.wait_for_connection().await;

        info!("BLE connected!");

        // Create BLE HID writers
        let mut keyboard_writer = ble_server.input_keyboard;
        let mut media_writer = ble_server.input_media_keys;
        let mut system_writer = ble_server.input_system_keys;
        let mut mouse_writer = ble_server.input_mouse_keys;

        let disconnect = BleServer::wait_for_disconnection(ble_server.server);

        let keyboard_fut = keyboard_task(&mut keyboard);
        let ble_fut = ble_communication_task(
            &keyboard_report_receiver,
            &mut keyboard_writer,
            &mut media_writer,
            &mut system_writer,
            &mut mouse_writer,
        );

        ble_server.output_vial.lock().on_write(|args| {
            let data: &[u8] = args.recv_data();
            debug!("BLE received {} {=[u8]:#X}", data.len(), data);
            block_on(via_output.send(unsafe { *(data.as_ptr() as *const [u8; 32]) }));
        });
        let mut via_rw = VialReaderWriter {
            receiver: via_output.receiver(),
            hid_writer: ble_server.input_vial,
        };
        let via_fut = vial_task(&mut via_rw, &mut vial_service);
        let matrix_fut = matrix.scan();
        let storage_fut = storage.run::<ROW, COL, NUM_LAYER>();
        pin_mut!(storage_fut);
        pin_mut!(via_fut);
        pin_mut!(keyboard_fut);
        pin_mut!(disconnect);
        pin_mut!(ble_fut);
        pin_mut!(matrix_fut);

        select4(
            select(storage_fut, keyboard_fut),
            select(disconnect, matrix_fut),
            ble_fut,
            via_fut,
        )
        .await;

        warn!("BLE disconnected!")
    }
}
