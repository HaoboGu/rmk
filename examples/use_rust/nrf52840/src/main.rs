#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use crate::keymap::{COL, NUM_LAYER, ROW};
use core::mem;
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::{self, InterruptExt, Priority},
    nvmc::Nvmc,
    pac,
    peripherals::{self, USBD},
    usb::{self, vbus_detect::HardwareVbusDetect, Driver},
};
use panic_probe as _;
use rmk::initialize_keyboard_and_run;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    POWER_CLOCK => usb::vbus_detect::InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let mut config = ::embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
    config.time_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
    ::embassy_nrf::interrupt::USBD.set_priority(::embassy_nrf::interrupt::Priority::P2);
    ::embassy_nrf::interrupt::POWER_CLOCK.set_priority(::embassy_nrf::interrupt::Priority::P2);
    let p = ::embassy_nrf::init(config);
    let clock: ::embassy_nrf::pac::CLOCK = unsafe { ::core::mem::transmute(()) };
    info!("Enabling ext hfosc...");
    clock.tasks_hfclkstart.write(|w| unsafe { w.bits(1) });
    while clock.events_hfclkstarted.read().bits() != 1 {}

    // Usb config
    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P0_07, P0_08, P0_11, P0_12], output: [P0_13, P0_14, P0_15]);

    // Use internal flash to emulate eeprom
    let f = Nvmc::new(p.NVMC);

    // Start serving
    initialize_keyboard_and_run::<
        Driver<'_, USBD, HardwareVbusDetect>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        Nvmc,
        ROW,
        COL,
        NUM_LAYER,
    >(
        driver,
        input_pins,
        output_pins,
        Some(f),
        crate::keymap::KEYMAP,
        VIAL_KEYBOARD_ID,
        VIAL_KEYBOARD_DEF,
    )
    .await;
}
