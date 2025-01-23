pub(crate) mod server;

use self::server::BleServer;
use crate::channel::VIAL_READ_CHANNEL;
use crate::config::RmkConfig;
use crate::light::LightController;
use crate::matrix::MatrixTrait;
use crate::storage::Storage;
use crate::KEYBOARD_STATE;
use crate::{keyboard::Keyboard, keymap::KeyMap};
use crate::{run_keyboard, CONNECTION_STATE};
use core::cell::RefCell;
use embedded_hal::digital::OutputPin;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use esp_idf_svc::hal::task::block_on;

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
/// * `spawner` - embassy task spawner, used to spawn nrf_softdevice background task
// TODO: add usb service for other chips of esp32 which have USB device
pub(crate) async fn run_esp_ble_keyboard<
    'a,
    M: MatrixTrait,
    F: AsyncNorFlash,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    keyboard: &mut Keyboard<'a, ROW, COL, NUM_LAYER>,
    matrix: &mut M,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER>,
    light_controller: &mut LightController<Out>,
    rmk_config: RmkConfig<'static>,
) -> ! {
    // esp32c3 doesn't have USB device, so there is no usb here
    loop {
        KEYBOARD_STATE.store(false, core::sync::atomic::Ordering::Release);
        CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
        info!("Advertising..");
        let mut ble_server = BleServer::new(rmk_config.usb_config);
        ble_server.output_keyboard.lock().on_write(|args| {
            let data: &[u8] = args.recv_data();
            debug!("output_keyboard {}, {}", data.len(), data[0]);
        });

        info!("Waitting for connection..");
        ble_server.wait_for_connection().await;

        info!("BLE connected!");
        CONNECTION_STATE.store(true, core::sync::atomic::Ordering::Release);

        // Create BLE HID writers
        let keyboard_writer = ble_server.get_keyboard_writer();
        let vial_reader_writer = ble_server.get_vial_reader_writer();
        let led_reader = ble_server.get_led_reader();

        let disconnect = BleServer::wait_for_disconnection(ble_server.server);

        ble_server.output_vial.lock().on_write(|args| {
            let data: &[u8] = args.recv_data();
            debug!("BLE received {} {=[u8]:#X}", data.len(), data);
            block_on(VIAL_READ_CHANNEL.send(unsafe { *(data.as_ptr() as *const [u8; 32]) }));
        });

        run_keyboard(
            keymap,
            keyboard,
            matrix,
            storage,
            disconnect,
            light_controller,
            led_reader,
            vial_reader_writer,
            keyboard_writer,
            rmk_config.vial_config,
        )
        .await;

        warn!("BLE disconnected!")
    }
}
