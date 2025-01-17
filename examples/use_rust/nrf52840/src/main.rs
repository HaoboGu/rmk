#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::InterruptExt,
    nvmc::Nvmc,
    peripherals,
    usb::{self, vbus_detect::HardwareVbusDetect, Driver},
};
use panic_probe as _;
use rmk::{
    config::{RmkConfig, VialConfig},
    run_rmk,
};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => usb::vbus_detect::InterruptHandler;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let mut config = ::embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
    config.time_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
    ::embassy_nrf::interrupt::USBD.set_priority(::embassy_nrf::interrupt::Priority::P2);
    ::embassy_nrf::interrupt::CLOCK_POWER.set_priority(::embassy_nrf::interrupt::Priority::P2);
    let p = ::embassy_nrf::init(config);
    info!("Enabling ext hfosc...");
    ::embassy_nrf::pac::CLOCK.tasks_hfclkstart().write_value(1);
    while ::embassy_nrf::pac::CLOCK.events_hfclkstarted().read() != 1 {}

    // Usb config
    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P0_07, P0_08, P0_11, P0_12], output: [P0_13, P0_14, P0_15]);

    // Use internal flash to emulate eeprom
    let f = Nvmc::new(p.NVMC);

    // Keyboard config
    let keyboard_config = RmkConfig {
        vial_config: VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF),
        ..Default::default()
    };

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
