#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use core::cell::RefCell;
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    flash::{Blocking, Flash},
    gpio::{AnyPin, Input, Output},
    peripherals::USB_OTG_FS,
    usb_otg::{Driver, InterruptHandler},
    Config,
};
use panic_probe as _;
use rmk::{eeprom::EepromStorageConfig, initialize_keyboard_and_run, keymap::KeyMap};
use static_cell::StaticCell;

use crate::keymap::{COL, NUM_LAYER, ROW};

bind_interrupts!(struct Irqs {
    OTG_FS => InterruptHandler<USB_OTG_FS>;
});

const FLASH_SECTOR_7_ADDR: u32 = 0x60000;
const EEPROM_SIZE: usize = 128;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("start!");
    // RCC config
    let config = Config::default();

    // Initialize peripherals
    let p = embassy_stm32::init(config);

    // Usb config
    static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
    let mut usb_config = embassy_stm32::usb_otg::Config::default();
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
    let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);

    // Keymap + eeprom config
    static MY_KEYMAP: StaticCell<
        RefCell<KeyMap<Flash<'_, Blocking>, EEPROM_SIZE, ROW, COL, NUM_LAYER>>,
    > = StaticCell::new();
    let eeprom_storage_config = EepromStorageConfig {
        start_addr: FLASH_SECTOR_7_ADDR,
        storage_size: 0x20000, // uses last sector, 128KB for eeprom
        page_size: 8,
    };
    // Use internal flash to emulate eeprom
    let f = Flash::new_blocking(p.FLASH);
    let keymap = MY_KEYMAP.init(RefCell::new(KeyMap::new(
        crate::keymap::KEYMAP,
        Some(f),
        eeprom_storage_config,
        None,
    )));

    // Initialize all utilities: keyboard, usb and keymap
    initialize_keyboard_and_run::<
        Driver<'_, USB_OTG_FS>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        Flash<'_, Blocking>,
        EEPROM_SIZE,
        ROW,
        COL,
        NUM_LAYER,
    >(
        driver,
        input_pins,
        output_pins,
        keymap,
        &vial::VIAL_KEYBOARD_ID,
        &vial::VIAL_KEYBOARD_DEF,
    ).await;
}
