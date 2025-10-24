#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use defmt::info;
use embassy_executor::Spawner;
use keymap::{COL, ROW};
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
use sifli_hal::bind_interrupts;
use sifli_hal::gpio::{Input, Output};
use sifli_hal::rcc::{ClkSysSel, ConfigOption, DllConfig, UsbConfig, UsbSel};
use sifli_hal::usb::{Driver, InterruptHandler};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBC => InterruptHandler<sifli_hal::peripherals::USBC>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Hello World! RMK SF32LB52x USB example");
    let mut config = sifli_hal::Config::default();
    // 240MHz Dll1 Freq = (stg + 1) * 24MHz
    config.rcc.dll1 = ConfigOption::Update(DllConfig {
        enable: true,
        stg: 9,
        div2: false,
    });
    config.rcc.clk_sys_sel = ConfigOption::Update(ClkSysSel::Dll1);
    config.rcc.usb = ConfigOption::Update(UsbConfig {
        sel: UsbSel::ClkSys,
        div: 4,
    });
    let p = sifli_hal::init(config);

    sifli_hal::rcc::test_print_clocks();

    let driver = Driver::new(p.USBC, Irqs, p.PA35, p.PA36);

    // Pin config
    let (row_pins, col_pins) =
        config_matrix_pins_sifli!(peripherals: p, input: [PA9, PA6, PA1, PA33], output: [PA3, PA4, PA2]);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "RMK & SiFli-rs",
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
