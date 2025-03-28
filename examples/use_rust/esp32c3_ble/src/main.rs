#![feature(type_alias_impl_trait)]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::info;
use esp_idf_svc::hal::gpio::*;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::task::block_on;
use esp_idf_svc::partition::EspPartition;
use esp_println as _;
use keymap::{COL, ROW};
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{ControllerConfig, RmkConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::light::LightController;
use rmk::matrix::Matrix;
use rmk::storage::async_flash_wrapper;
use rmk::{initialize_keymap_and_storage, run_devices, run_rmk};

use crate::vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

fn main() {
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Hello ESP BLE!");
    let peripherals = Peripherals::take().unwrap();

    // Pin config
    // WARNING: Some gpio pins shouldn't be used, the initial state is error.
    // reference: table 2-3 in https://www.espressif.com.cn/sites/default/files/documentation/esp32-c3_datasheet_en.pdf
    let (input_pins, output_pins) = config_matrix_pins_esp!(peripherals: peripherals , input: [gpio6, gpio7, gpio20, gpio21], output: [gpio3, gpio4, gpio5]);

    // Keyboard config
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let rmk_config = RmkConfig {
        vial_config,
        ..Default::default()
    };

    let flash = async_flash_wrapper(unsafe {
        EspPartition::new("rmk")
            .expect("Create storage partition error")
            .expect("Empty partition")
    });

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let (keymap, storage) = block_on(initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        rmk_config.storage_config,
        rmk_config.behavior_config.clone(),
    ));

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());

    // Initialize the light controller
    let light_controller: LightController<PinDriver<AnyOutputPin, Output>> =
        LightController::new(ControllerConfig::default().light_config);

    // Start
    block_on(join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(&keymap, storage, light_controller, rmk_config),
    ));
}
