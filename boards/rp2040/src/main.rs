#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use crate::keymap::{COL, NUM_LAYER, ROW};
use core::cell::RefCell;
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    flash::{Blocking, Flash},
    gpio::{AnyPin, Input, Output},
    peripherals::{self, USB},
    usb::{Driver, InterruptHandler},
};
use panic_probe as _;
use rmk::{initialize_keyboard_and_run, keymap::KeyMap};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const EEPROM_SIZE: usize = 128;
const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Create the usb driver, from the HAL
    let driver = Driver::new(p.USB, Irqs);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7, PIN_8, PIN_9], output: [PIN_19, PIN_20, PIN_21]);

    // Use internal flash to emulate eeprom
    let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    // Keymap + eeprom config
    static MY_KEYMAP: StaticCell<
        RefCell<
            KeyMap<
                Flash<peripherals::FLASH, Blocking, FLASH_SIZE>,
                EEPROM_SIZE,
                ROW,
                COL,
                NUM_LAYER,
            >,
        >,
    > = StaticCell::new();
    let keymap = MY_KEYMAP.init(RefCell::new(KeyMap::new(
        crate::keymap::KEYMAP,
        Some(flash),
        None,
    )));

    // Initialize all utilities: keyboard, usb and keymap
    initialize_keyboard_and_run::<
        Driver<'_, USB>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        Flash<peripherals::FLASH, Blocking, FLASH_SIZE>,
        EEPROM_SIZE,
        ROW,
        COL,
        NUM_LAYER,
    >(
        driver,
        input_pins,
        output_pins,
        keymap,
        &VIAL_KEYBOARD_ID,
        &VIAL_KEYBOARD_DEF,
    )
    .await;
}
