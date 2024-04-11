#![feature(type_alias_impl_trait)]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use crate::{
    keymap::{COL, NUM_LAYER, ROW},
    vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID},
};
use defmt::*;
use esp_idf_svc::hal::{gpio::*, peripherals::Peripherals};
use esp_idf_svc::hal::task::block_on;
use esp_println as _;
use rmk::{
    config::{RmkConfig, VialConfig},
    initialize_esp_ble_keyboard_with_config_and_run,
};

fn main() {
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Hello ESP BLE!");
    let peripherals = Peripherals::take().unwrap();

    // Pin config
    // WARNING: Some gpio pins shouldn't be used, the initial state is error.
    // reference: table 2-3 in https://www.espressif.com.cn/sites/default/files/documentation/esp32-c3_datasheet_en.pdf
    let (input_pins, output_pins) = config_matrix_pins_esp!(peripherals: peripherals , input: [gpio6, gpio7, gpio20, gpio21], output: [gpio3, gpio4, gpio5]);

    // Keyboard config
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let keyboard_config = RmkConfig {
        vial_config,
        ..Default::default()
    };

    // Start serving
    block_on(initialize_esp_ble_keyboard_with_config_and_run::<
        PinDriver<'_, AnyInputPin, Input>,
        PinDriver<'_, AnyOutputPin, Output>,
        ROW,
        COL,
        NUM_LAYER,
    >(
        crate::keymap::KEYMAP,
        input_pins,
        output_pins,
        keyboard_config,
    ));
}
