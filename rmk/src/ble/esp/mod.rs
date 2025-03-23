pub(crate) mod server;

use self::server::BleServer;
use crate::channel::VIAL_READ_CHANNEL;
use crate::config::RmkConfig;
use crate::keymap::KeyMap;
use crate::light::LightController;
use crate::storage::Storage;
use crate::{run_keyboard, CONNECTION_STATE};
use core::cell::RefCell;
use embedded_hal::digital::OutputPin;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;

/// Initialize and run the BLE keyboard service, with given keyboard usb config.
/// Can only be used on nrf52 series microcontrollers with `nrf-softdevice` crate.
/// This function never returns.
///
/// # Arguments
///
/// * `keymap` - default keymap definition
/// * `storage` - storage for saving keymap and other data
/// * `light_controller` - light controller for controlling the light
/// * `rmk_config` - other configurations of the keyboard, check [RmkConfig] struct for details
// TODO: add usb service for other chips of esp32 which have USB device
pub(crate) async fn run_esp_ble_keyboard<
    'a,
    F: AsyncNorFlash,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    light_controller: &mut LightController<Out>,
    rmk_config: RmkConfig<'static>,
) -> ! {
    // esp32c3 doesn't have USB device, so there is no usb here
    loop {
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

        run_keyboard(
            keymap,
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
