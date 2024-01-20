#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use core::cell::RefCell;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::{
    bind_interrupts,
    flash::{Blocking, Flash},
    gpio::{AnyPin, Input, Output},
    peripherals::{self, USB},
    usb::{Driver, InterruptHandler},
};
use embassy_time::Timer;

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;
use rmk::{eeprom::EepromStorageConfig, initialize_keyboard_and_usb_device, keymap::KeyMap};
use static_cell::StaticCell;

use crate::keymap::{COL, NUM_LAYER, ROW};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

// static SUSPENDED: AtomicBool = AtomicBool::new(false);
const FLASH_SECTOR_15_ADDR: u32 = 15 * 8192;
const EEPROM_SIZE: usize = 128;
const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Rmk start!");
    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Create the usb driver, from the HAL
    let driver = Driver::new(p.USB, Irqs);

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7, PIN_8, PIN_9], output: [PIN_19, PIN_20, PIN_21]);

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
    let eeprom_storage_config = EepromStorageConfig {
        start_addr: FLASH_SECTOR_15_ADDR,
        storage_size: 8192, // uses 8KB for eeprom
        page_size: 32,
    };
    // Use internal flash to emulate eeprom
    // let f = Flash::new_blocking(p.FLASH);
    let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    // let mut flash = Flash::new_blocking(p.FLASH);
    let keymap = MY_KEYMAP.init(RefCell::new(KeyMap::new(
        crate::keymap::KEYMAP,
        Some(flash),
        eeprom_storage_config,
        None,
    )));

    // Initialize all utilities: keyboard, usb and keymap
    let (mut keyboard, mut usb_device, vial) = initialize_keyboard_and_usb_device::<
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
        &vial::VIAL_KEYBOARD_ID,
        &vial::VIAL_KEYBOARD_DEF,
    );

    let usb_fut = usb_device.device.run();
    let keyboard_fut = async {
        loop {
            let _ = keyboard.keyboard_task().await;
            keyboard.send_report(&mut usb_device.keyboard_hid).await;
            keyboard.send_media_report(&mut usb_device.other_hid).await;
        }
    };

    let via_fut = async {
        loop {
            vial.process_via_report(&mut usb_device.via_hid).await;
            Timer::after_millis(1).await;
        }
    };
    join(usb_fut, join(keyboard_fut, via_fut)).await;
}
