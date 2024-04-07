pub(crate) mod server;

use self::server::BleServer;
use crate::{
    action::KeyAction, ble::keyboard_ble_task, config::RmkConfig, flash::EmptyFlashWrapper,
    keyboard::Keyboard, keymap::KeyMap,
};
use core::cell::RefCell;
use defmt::{info, warn};
use embassy_futures::join::join;
use embedded_hal::digital::{InputPin, OutputPin};

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
pub async fn initialize_esp_ble_keyboard_with_config_and_run<
    // F: NorFlash,
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    // flash: Option<F>,
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // TODO: Use esp nvs as the storage
    let (mut _storage, keymap) = (
        None::<EmptyFlashWrapper>,
        RefCell::new(
            KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage::<EmptyFlashWrapper>(keymap, None)
                .await,
        ),
    );

    let mut keyboard = Keyboard::new(input_pins, output_pins, &keymap);
    // esp32c3 doesn't have USB device, so there is no usb here
    // TODO: add usb service for other chips of esp32 which have USB device

    loop {
        info!("Advertising..");
        let mut ble_server = BleServer::new(keyboard_config.usb_config);
        info!("Waitting for connection..");
        ble_server.wait_for_connection().await;

        info!("BLE connected!");

        // Create BLE HID writers
        let mut keyboard_writer = ble_server.input_keyboard;
        let mut media_writer = ble_server.input_media_keys;
        let mut system_writer = ble_server.input_system_keys;
        let mut mouse_writer = ble_server.input_mouse_keys;

        let disconnect = BleServer::wait_for_disconnection(ble_server.server);
        let keyboard_fut = keyboard_ble_task(
            &mut keyboard,
            &mut keyboard_writer,
            &mut media_writer,
            &mut system_writer,
            &mut mouse_writer,
        );
        join(keyboard_fut, disconnect).await;

        warn!("BLE disconnected!")
    }
}