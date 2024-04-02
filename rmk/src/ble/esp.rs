pub(crate) mod server;

use self::server::BleServer;
use crate::{
    action::KeyAction, config::RmkConfig, keyboard::Keyboard, keymap::KeyMap, storage::Storage,
};
use core::cell::RefCell;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;

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
    F: NorFlash,
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    flash: Option<F>,
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Wrap `embedded-storage` to `embedded-storage-async`
    let async_flash = flash.map(|f| embassy_embedded_hal::adapter::BlockingAsync::new(f));

    // Initialize storage and keymap
    let (mut storage, keymap) = match async_flash {
        Some(f) => {
            let mut s = Storage::new(f, &keymap, keyboard_config.storage_config).await;
            let keymap = RefCell::new(
                KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(keymap, Some(&mut s)).await,
            );
            (Some(s), keymap)
        }
        None => {
            let keymap = RefCell::new(
                KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage::<BlockingAsync<F>>(keymap, None)
                    .await,
            );
            (None, keymap)
        }
    };

    let keyboard = Keyboard::new(input_pins, output_pins, &keymap);
    let ble_server = BleServer::new(keyboard_config.usb_config);

    let adv_fut = async {
        loop {
            Timer::after_millis(10).await;
            if !ble_server.connected() {
                continue;
            }
            break;
        }
    };

    adv_fut.await;

    loop {}
}
