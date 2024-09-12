#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    self as _,
    gpio::{AnyPin, Input, Output},
    interrupt::Priority,
};
use panic_probe as _;
use rmk::{
    config::{KeyboardUsbConfig, RmkConfig, StorageConfig, VialConfig},
    run_rmk,
};

use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello NRF BLE!");

    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.gpiote_interrupt_priority = Priority::P2;
    nrf_config.time_interrupt_priority = Priority::P2;
    let p = embassy_nrf::init(nrf_config);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P0_03, P0_04, P0_28, P0_29], output: [P0_07, P0_11, P0_27]);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "00000000",
    };
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    // Current default storage config of nRF52832 is not correct, check this issue: https://github.com/embassy-rs/nrf-softdevice/issues/246.
    // So we set the storage config manually
    let storage_config = StorageConfig {
        start_addr: 0x70000,
        num_sectors: 2,
    };
    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        storage_config,
        ..Default::default()
    };

    run_rmk(
        input_pins,
        output_pins,
        crate::keymap::KEYMAP,
        keyboard_config,
        spawner,
    )
    .await;
}
