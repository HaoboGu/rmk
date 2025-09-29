#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Output};
use embassy_stm32::peripherals::USB;
use embassy_stm32::usb::{Driver, InterruptHandler};
use embassy_stm32::{Config, bind_interrupts};
use keymap::{COL, ROW};
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, PositionalConfig, RmkConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::matrix::Matrix;
use rmk::{initialize_keymap, run_devices, run_rmk};
use {defmt_rtt as _, panic_halt as _};
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
    let (input_pins, output_pins) =
        config_matrix_pins_stm32!(peripherals: p, input: [PB9, PB8, PB13, PB12], output: [PA8, PA9, PA10]);

    // Usb driver
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    // Keyboard config
    let rmk_config = RmkConfig::default();

    // Initialize the keymap
    let mut default_keymap = keymap::get_default_keymap();
    let mut behavior_config = BehaviorConfig::default();
    // let storage_config = StorageConfig::default();
    let mut per_key_config = PositionalConfig::default();
    let keymap = initialize_keymap(&mut default_keymap, &mut behavior_config, &mut per_key_config).await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);

    // Start
    join3(
        keyboard.run(),
        run_rmk(driver, rmk_config),
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
    )
    .await;
}
