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
// use py32_hal::flash::Flash;
use py32_hal::gpio::{Input, Output};
use py32_hal::rcc::{HsiFs, Pll, PllMul, PllSource, Sysclk};
use py32_hal::usb::{Driver, InterruptHandler};
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, KeyboardUsbConfig, PositionalConfig, RmkConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
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
    let (row_pins, col_pins) =
        config_matrix_pins_py!(peripherals: p, input: [PA0, PA1, PA2, PA3], output: [PA4, PA5, PA6]);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "RMK & py32-rs",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };

    let _vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, &[(0, 0), (1, 1)]);
    // let storage_config = rmk::config::StorageConfig::default();

    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        // vial_config,
        ..Default::default()
    };

    // let f = Flash::new_blocking(p.MPI2);

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let mut behavior_config = BehaviorConfig::default();
    // let storage_config = StorageConfig::default();
    let mut per_key_config = PositionalConfig::default();
    let keymap = rmk::initialize_keymap(&mut default_keymap, &mut behavior_config, &mut per_key_config).await;
    // let (keymap, mut storage) = initialize_keymap_and_storage(
    //     &mut default_keymap,
    //     async_flash_wrapper(f),
    //     &storage_config,
    //     &mut behavior_config,
    //     &mut per_key_config,
    // )
    // .await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL, true>::new(row_pins, col_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);

    // Start
    join3(
        run_devices!((matrix) => EVENT_CHANNEL),
        keyboard.run(),
        // run_rmk(driver, &mut storage, rmk_config),
        run_rmk(driver, rmk_config),
    )
    .await;
}
