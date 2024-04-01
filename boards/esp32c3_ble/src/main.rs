#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_closure)]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use crate::{
    keymap::{COL, NUM_LAYER, ROW},
    vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID},
};
use defmt::*;
use embassy_executor::Spawner;
use esp_idf_hal::{gpio::*, peripherals::Peripherals};
use esp_idf_sys as _;
use rmk::{
    ble::esp::initialize_esp_ble_keyboard_with_config_and_run, config::{KeyboardUsbConfig, RmkConfig, VialConfig}
    // initialize_esp_ble_keyboard_with_config_and_run,
};

pub const SOC_NAME: &str = "ESP32-C3";
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello ESP BLE!");
    let peripherals = Peripherals::take().unwrap();

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_esp!(peripherals: peripherals , input: [gpio6, gpio7, gpio8, gpio9], output: [gpio10, gpio11, gpio12]);

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let keyboard_config = RmkConfig {
        vial_config,
        ..Default::default()
    };

    initialize_esp_ble_keyboard_with_config_and_run::<
        PinDriver<'_, AnyIOPin, Input>,
        PinDriver<'_, AnyIOPin, Output>,
        ROW,
        COL,
        NUM_LAYER,
    >(
        crate::keymap::KEYMAP,
        input_pins,
        output_pins,
        keyboard_config,
    )
    .await;
}
