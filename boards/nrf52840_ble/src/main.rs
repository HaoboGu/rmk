#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use crate::keymap::{COL, NUM_LAYER, ROW};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    self as _,
    gpio::{AnyPin, Input, Output},
    interrupt::Priority,
    nvmc::Nvmc,
};
use panic_probe as _;
use rmk::{
    ble::nrf_ble_config,
    config::{KeyboardUsbConfig, RmkConfig, VialConfig},
};

use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

const EEPROM_SIZE: usize = 128;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello NRF BLE!");
    let keyboard_name = "RMK Keyboard";
    let ble_config = nrf_ble_config(keyboard_name);

    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.gpiote_interrupt_priority = Priority::P2;
    nrf_config.time_interrupt_priority = Priority::P2;
    let p = embassy_nrf::init(nrf_config);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P1_00, P1_01, P1_02, P1_03], output: [P1_05, P1_06, P1_07]);

    let keyboard_usb_config = KeyboardUsbConfig::new(
        0x4c4b,
        0x4643,
        Some("Haobo"),
        Some(keyboard_name),
        Some("00000001"),
    );
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    rmk::initialize_ble_keyboard_with_config_and_run::<
        Nvmc<'_>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        EEPROM_SIZE,
        ROW,
        COL,
        NUM_LAYER,
    >(
        crate::keymap::KEYMAP,
        input_pins,
        output_pins,
        ble_config,
        keyboard_config,
        spawner,
    )
    .await;
}
