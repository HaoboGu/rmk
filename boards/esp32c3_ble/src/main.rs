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
use esp_idf_svc::hal::{gpio::*, peripherals::Peripherals};
use esp_idf_svc::nvs::*;
use esp_println as _;
use rmk::{
    ble::esp::initialize_esp_ble_keyboard_with_config_and_run,
    config::{RmkConfig, VialConfig}, // initialize_esp_ble_keyboard_with_config_and_run,
};
pub const SOC_NAME: &str = "ESP32-C3";

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Hello ESP BLE!");
    let peripherals = Peripherals::take().unwrap();

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_esp!(peripherals: peripherals , input: [gpio6, gpio7, gpio8, gpio9], output: [gpio10, gpio11, gpio12]);

    // Flash config
    let nvs_default_partition: EspNvsPartition<NvsDefault> =
        EspDefaultNvsPartition::take().unwrap();
    let test_namespace = "test_ns";
    let mut _nvs = match EspNvs::new(nvs_default_partition, test_namespace, true) {
        Ok(nvs) => {
            info!("Got namespace {:?} from default partition", test_namespace);
            nvs
        }
        Err(_e) => defmt::panic!("Could't get namespace"),
    };

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
