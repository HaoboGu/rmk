#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use core::cell::RefCell;

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    flash::{Async, Flash},
    gpio::{AnyPin, Input, Output},
    peripherals::USB,
    usb::{Driver, InterruptHandler},
};
use keymap::{get_default_keymap, COL, ROW};
// use embassy_rp::flash::Blocking;
use panic_probe as _;
use rmk::{
    bind_device_and_processor_and_run,
    config::{ControllerConfig, KeyboardUsbConfig, RmkConfig, VialConfig},
    debounce::{default_bouncer::DefaultDebouncer, DebouncerTrait},
    futures::future::join,
    input_device::{InputDevice, InputProcessor},
    keyboard::Keyboard,
    keymap::KeyMap,
    light::LightController,
    matrix::Matrix,
    run_rmk,
    storage::Storage,
};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Create the usb driver, from the HAL
    let driver = Driver::new(p.USB, Irqs);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7, PIN_8, PIN_9], output: [PIN_19, PIN_20, PIN_21]);

    // Use internal flash to emulate eeprom
    // Both blocking and async flash are support, use different API
    // let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    let flash = Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    // 1. Create the storage + keymap
    let mut default_keymap = keymap::get_default_keymap();
    let (keymap, storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        rmk_config.storage_config,
        rmk_config.behavior_config.clone(),
    )
    .await;

    // 2. Create the matrix + keyboard
    // Create the debouncer, use COL2ROW by default
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    // Keyboard matrix, use COL2ROW by default
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());

    // 3. Create the light controller
    let light_controller: LightController<Output> =
        LightController::new(ControllerConfig::default().light_config);

    // Start serving
    join(
        bind_device_and_processor_and_run!((matrix) => keyboard),
        run_rmk(&keymap, driver, storage, light_controller, rmk_config),
    )
    .await;
}
