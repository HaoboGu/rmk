#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    Config, bind_interrupts,
    flash::Flash,
    gpio::{Input, Output},
    peripherals::USB_OTG_FS,
    usb::{Driver, InterruptHandler},
};
use panic_probe as _;
use rmk::{
    config::{KeyboardConfig, RmkConfig, VialConfig},
    run_rmk,
};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    OTG_FS => InterruptHandler<USB_OTG_FS>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("RMK start!");
    // RCC config
    let config = Config::default();

    // Initialize peripherals
    let p = embassy_stm32::init(config);

    // Usb config
    static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
    let mut usb_config = embassy_stm32::usb::Config::default();
    usb_config.vbus_detection = false;
    let driver = Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut EP_OUT_BUFFER.init([0; 1024])[..],
        usb_config,
    );

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PA9, PB8, PB13, PB12], output: [PA13, PA14, PA15]);

    // Use internal flash to emulate eeprom
    let f = Flash::new_blocking(p.FLASH);

    // Keyboard config
    let rmk_config = RmkConfig {
        vial_config: VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF),
        ..Default::default()
    };

    // Keyboard config
    let keyboard_config = KeyboardConfig {
        rmk_config,
        ..Default::default()
    };

    // Start serving
    run_rmk(
        input_pins,
        output_pins,
        driver,
        f,
        &mut keymap::get_default_keymap(),
        keyboard_config,
        spawner,
    )
    .await;
}
