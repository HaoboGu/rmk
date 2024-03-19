#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use crate::keymap::{COL, NUM_LAYER, ROW};
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    flash::{Blocking, Flash},
    gpio::{AnyPin, Input, Level, Output, Speed},
    peripherals::USB,
    usb::{Driver, InterruptHandler},
    Config,
};
use embassy_time::Timer;
use panic_halt as _;
use rmk::initialize_keyboard_and_run;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

#[defmt::global_logger]
struct Logger;

unsafe impl defmt::Logger for Logger {
    fn acquire() {}
    unsafe fn flush() {}
    unsafe fn release() {}
    unsafe fn write(_bytes: &[u8]) {}
}

bind_interrupts!(struct Irqs {
    USB_LP_CAN1_RX0 => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // RCC config
    let config = Config::default();

    // Initialize peripherals
    let mut p = embassy_stm32::init(config);

    {
        // BluePill board has a pull-up resistor on the D+ line.
        // Pull the D+ pin down to send a RESET condition to the USB bus.
        // This forced reset is needed only for development, without it host
        // will not reset your device when you upload new firmware.
        let _dp = Output::new(&mut p.PA12, Level::Low, Speed::Low);
        Timer::after_millis(10).await;
    }

    // Usb driver
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    // Pin config
    // TODO: fix pins
    let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);

    // Use internal flash to emulate eeprom
    let f = Flash::new_blocking(p.FLASH);

    // Start serving
    initialize_keyboard_and_run::<
        Driver<'_, USB>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        Flash<'_, Blocking>,
        ROW,
        COL,
        NUM_LAYER,
    >(
        driver,
        input_pins,
        output_pins,
        Some(f),
        crate::keymap::KEYMAP,
        &VIAL_KEYBOARD_ID,
        &VIAL_KEYBOARD_DEF,
    )
    .await;
}
