#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use embassy_executor::Spawner;
use keymap::{COL, ROW};
use py32_hal::bind_interrupts;
use py32_hal::flash::Flash;
use py32_hal::gpio::{Input, Output};
use py32_hal::rcc::{HsiFs, Pll, PllMul, PllSource, Sysclk};
use py32_hal::usb::{Driver, InterruptHandler};
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, ControllerConfig, KeyboardUsbConfig, RmkConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::light::LightController;
use rmk::matrix::Matrix;
// use rmk::storage::async_flash_wrapper;
// use rmk::{initialize_keymap_and_storage, run_devices, run_rmk};
use rmk::{run_devices, run_rmk};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USB => InterruptHandler<py32_hal::peripherals::USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut cfg: py32_hal::Config = Default::default();

    // PY32 USB uses PLL as the clock source and can only run at 48Mhz.
    cfg.rcc.hsi = Some(HsiFs::HSI_16MHZ);
    cfg.rcc.pll = Some(Pll {
        src: PllSource::HSI,
        mul: PllMul::MUL3,
    });
    cfg.rcc.sys = Sysclk::PLL;
    let p = py32_hal::init(cfg);

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_py!(peripherals: p, input: [PA0, PA1, PA2, PA3], output: [PA4, PA5, PA6]);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    // let storage_config = rmk::config::StorageConfig::default();

    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    let f = Flash::new_blocking(p.FLASH);

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let behavior_config = BehaviorConfig::default();
    let keymap = rmk::initialize_keymap(&mut default_keymap, behavior_config).await;
    // let (keymap, mut storage) = initialize_keymap_and_storage(
    //     &mut default_keymap,
    //     async_flash_wrapper(f),
    //     &storage_config,
    //     behavior_config,
    // )
    // .await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    // let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut matrix = rmk::matrix::TestMatrix::<ROW, COL>::new();
    let mut keyboard = Keyboard::new(&keymap);

    // Initialize the light controller
    let mut light_controller: LightController<Output> = LightController::new(ControllerConfig::default().light_config);

    // Start
    join3(
        run_devices!((matrix) => EVENT_CHANNEL),
        keyboard.run(),
        // run_rmk(&keymap, driver, &mut storage, &mut light_controller, rmk_config),
        run_rmk(&keymap, driver, &mut light_controller, rmk_config),
    )
    .await;
}
