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
    self as _, bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::{self, InterruptExt, Priority},
    peripherals::{self, USBD},
    usb::{self, vbus_detect::SoftwareVbusDetect, Driver},
};
use panic_probe as _;
use rmk::{
    ble::SOFTWARE_VBUS,
    config::{KeyboardUsbConfig, RmkConfig, VialConfig},
    initialize_nrf_ble_keyboard_with_config_and_run,
};

use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello NRF BLE!");
    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.gpiote_interrupt_priority = Priority::P3;
    nrf_config.time_interrupt_priority = Priority::P3;
    let p = embassy_nrf::init(nrf_config);
    interrupt::USBD.set_priority(interrupt::Priority::P2);
    interrupt::POWER_CLOCK.set_priority(interrupt::Priority::P2);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P1_00, P1_01, P1_02, P1_03], output: [P1_05, P1_06, P1_07]);

    // Usb config
    let software_vbus = SOFTWARE_VBUS.get_or_init(|| SoftwareVbusDetect::new(true, false));
    let driver = Driver::new(p.USBD, Irqs, software_vbus);

    // Keyboard config
    let keyboard_usb_config = KeyboardUsbConfig::new(
        0x4c4b,
        0x4643,
        Some("Haobo"),
        Some("RMK Keyboard"),
        Some("00000001"),
    );
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    initialize_nrf_ble_keyboard_with_config_and_run::<
        Driver<'_, USBD, &SoftwareVbusDetect>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        ROW,
        COL,
        NUM_LAYER,
    >(
        crate::keymap::KEYMAP,
        input_pins,
        output_pins,
        Some(driver),
        keyboard_config,
        spawner,
    )
    .await;
}
