#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

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
use keymap::{COL, ROW};
use panic_probe as _;
use rmk::{
    channel::EVENT_CHANNEL,
    config::{ControllerConfig, KeyboardUsbConfig, RmkConfig, VialConfig},
    debounce::default_debouncer::DefaultDebouncer,
    futures::future::join3,
    initialize_keymap_and_storage,
    input_device::Runnable,
    keyboard::Keyboard,
    light::LightController,
    matrix::Matrix,
    run_devices, run_rmk,
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
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());

    // Initialize the light controller
    let light_controller: LightController<Output> =
        LightController::new(ControllerConfig::default().light_config);

    // Start
    join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(&keymap, driver, storage, light_controller, rmk_config),
    )
    .await;
}
