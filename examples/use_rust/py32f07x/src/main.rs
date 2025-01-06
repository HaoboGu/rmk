#![no_main]
#![no_std]
#![feature(impl_trait_in_assoc_type)]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use defmt_rtt as _;
use embassy_executor::Spawner;
use py32_hal::{
    bind_interrupts,
    gpio::{AnyPin, Input, Output},
    rcc::{Pll, PllMul, PllSource, Sysclk},
    time::Hertz,
    usb::{Driver, InterruptHandler},
};
// use py32_hal::flash::Blocking;
use panic_probe as _;
use rmk::{
    config::{KeyboardUsbConfig, RmkConfig, VialConfig},
    run_rmk,
};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USB => InterruptHandler<py32_hal::peripherals::USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut cfg: py32_hal::Config = Default::default();

    // PY32 USB uses PLL as the clock source and can only run at 48Mhz.
    cfg.rcc.hsi = Some(Hertz::mhz(16));
    cfg.rcc.pll = Some(Pll {
        src: PllSource::HSI,
        mul: PllMul::MUL3,
    });
    cfg.rcc.sys = Sysclk::PLL;
    let p = py32_hal::init(cfg);

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_py!(peripherals: p, input: [PA0, PA1, PA2, PA3], output: [PA4, PA5, PA6]);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    // Start serving
    // Use `run_rmk` for blocking flash
    run_rmk(
        input_pins,
        output_pins,
        driver,
        // flash,
        &mut keymap::get_default_keymap(),
        keyboard_config,
        spawner,
    )
    .await;
}
