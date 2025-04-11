#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    gpio::{Input, Output},
    peripherals::USB,
    usb::{Driver, InterruptHandler},
    Config,
};
use keymap::{COL, ROW};
use panic_halt as _;
use rmk::{
    channel::EVENT_CHANNEL,
    config::{ControllerConfig, RmkConfig, VialConfig},
    debounce::default_debouncer::DefaultDebouncer,
    futures::future::join3,
    initialize_keymap,
    input_device::Runnable,
    keyboard::Keyboard,
    light::LightController,
    matrix::Matrix,
    run_devices, run_rmk,
};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
bind_interrupts!(struct Irqs {
    USB_LP => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // RCC config
    let config = Config::default();

    // Initialize peripherals
    let p = embassy_stm32::init(config);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PB9, PB8, PB13, PB12], output: [PA8, PA9, PA10]);

    // Usb driver
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    // Keyboard config
    let rmk_config = RmkConfig {
        vial_config: VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF),
        ..Default::default()
    };

    // Initialize the keymap
    let mut default_keymap = keymap::get_default_keymap();
    let keymap = initialize_keymap(&mut default_keymap, rmk_config.behavior_config.clone()).await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());

    // Initialize the light controller
    let light_controller: LightController<Output> =
        LightController::new(ControllerConfig::default().light_config);

    // Start
    join3(
        keyboard.run(),
        run_rmk(&keymap, driver, light_controller, rmk_config),
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
    )
    .await;
}
