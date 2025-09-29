#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Speed};
use embassy_stm32::peripherals::USB;
use embassy_stm32::usb::{Driver, InterruptHandler};
use embassy_stm32::{Config, bind_interrupts};
use embassy_time::Timer;
use keymap::{COL, ROW};
use panic_halt as _;
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, PositionalConfig, RmkConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::matrix::Matrix;
use rmk::{initialize_keymap, run_devices, run_rmk};

bind_interrupts!(struct Irqs {
    USB_LP_CAN1_RX0 => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // RCC config
    let config = Config::default();

    // Initialize peripherals
    let mut p = embassy_stm32::init(config);

    {
        // BluePill board has a pull-up resistor on the D+ line.
        // Pull the D+ pin down to send a RESET condition to the USB bus.
        // This forced reset is needed only for development, without it host
        // will not reset your device when you upload new firmware.
        let _dp = Output::new(p.PA12.reborrow(), Level::Low, Speed::Low);
        Timer::after_millis(10).await;
    }
    // Usb driver
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_stm32!(peripherals: p, input: [PA9, PB8, PB13, PB12], output: [PA13, PA14, PA15]);

    // Keyboard config
    let rmk_config = RmkConfig { ..Default::default() };

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let mut behavior_config = BehaviorConfig::default();
    let mut per_key_config = PositionalConfig::default();
    let keymap = initialize_keymap(&mut default_keymap, &mut behavior_config, &mut per_key_config).await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);

    // Start
    join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(driver, rmk_config),
    )
    .await;
}
