#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{AnyPin, Input, Output};
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{self as _};
use keymap::{COL, ROW};
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{ControllerConfig, KeyboardUsbConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::light::LightController;
use rmk::matrix::Matrix;
use rmk::{initialize_keymap_and_storage, initialize_nrf_sd_and_flash, run_devices, run_rmk};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello NRF BLE!");

    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.gpiote_interrupt_priority = Priority::P2;
    nrf_config.time_interrupt_priority = Priority::P2;
    let p = embassy_nrf::init(nrf_config);

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_nrf!(peripherals: p, input: [P0_03, P0_04, P0_28, P0_29], output: [P0_07, P0_11, P0_27]);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    // Current default storage config of nRF52832 is not correct, check this issue: https://github.com/embassy-rs/nrf-softdevice/issues/246.
    // So we set the storage config manually
    let storage_config = StorageConfig {
        start_addr: 0x70000,
        num_sectors: 2,
        ..Default::default()
    };
    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        storage_config,
        ..Default::default()
    };

    // Initialize the Softdevice and flash
    let (sd, flash) = initialize_nrf_sd_and_flash(rmk_config.usb_config.product_name, spawner, None);

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let (keymap, storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        rmk_config.storage_config,
        rmk_config.behavior_config.clone(),
    )
    .await;

    // Initialize the matrix + keyboard
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    // let mut matrix = TestMatrix::<ROW, COL>::new();

    // Initialize the light controller
    let light_controller: LightController<Output> = LightController::new(ControllerConfig::default().light_config);

    // Start
    join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(&keymap, storage, light_controller, rmk_config, sd),
    )
    .await;
}
