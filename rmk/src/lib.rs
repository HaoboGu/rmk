#![doc = include_str!("../../README.md")]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]

use core::{cell::RefCell, convert::Infallible};
use embassy_futures::join::join;
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use keyboard::{Keyboard, KeyboardUsbConfig};
use keymap::KeyMap;
use usb::KeyboardUsbDevice;
use via::process::VialService;

pub mod action;
mod debounce;
mod eeprom;
mod flash;
pub mod keyboard;
pub mod keycode;
pub mod keymap;
mod layout_macro;
mod matrix;
mod usb;
mod via;

/// DEPRECIATED: Use `initialize_keyboard_and_run` instead.
/// Initialize keyboard core and keyboard usb device
pub(crate) fn initialize_keyboard_and_usb_device<
    D: Driver<'static>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    keymap: &'static RefCell<KeyMap<F, EEPROM_SIZE, ROW, COL, NUM_LAYER>>,
    vial_keyboard_Id: &'static [u8],
    vial_keyboard_def: &'static [u8],
) -> (
    Keyboard<'static, In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
    KeyboardUsbDevice<'static, D>,
    VialService<'static, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
) {
    (
        Keyboard::new(input_pins, output_pins, keymap),
        KeyboardUsbDevice::new(driver, KeyboardUsbConfig::default()),
        VialService::new(keymap, vial_keyboard_Id, vial_keyboard_def),
    )
}

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
pub async fn initialize_keyboard_with_config_and_run<
    'a,
    D: Driver<'static>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    keymap: &'static RefCell<KeyMap<F, EEPROM_SIZE, ROW, COL, NUM_LAYER>>,
    keyboard_config: KeyboardUsbConfig<'static>,
    vial_keyboard_Id: &'static [u8],
    vial_keyboard_def: &'static [u8],
) -> ! {
    let (mut keyboard, mut usb_device, vial_service) = (
        Keyboard::new(input_pins, output_pins, keymap),
        KeyboardUsbDevice::new(driver, keyboard_config),
        VialService::new(keymap, vial_keyboard_Id, vial_keyboard_def),
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
            vial_service
                .process_via_report(&mut usb_device.via_hid)
                .await;
            Timer::after_millis(1).await;
        }
    };
    join(usb_fut, join(keyboard_fut, via_fut)).await;

    panic!("Keyboard service is died")
}

/// Initialize and run the keyboard service, this function never returns.
pub async fn initialize_keyboard_and_run<
    D: Driver<'static>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    keymap: &'static RefCell<KeyMap<F, EEPROM_SIZE, ROW, COL, NUM_LAYER>>,
    vial_keyboard_Id: &'static [u8],
    vial_keyboard_def: &'static [u8],
) -> ! {
    let (mut keyboard, mut usb_device, vial_service) = (
        Keyboard::new(input_pins, output_pins, keymap),
        KeyboardUsbDevice::new(driver, KeyboardUsbConfig::default()),
        VialService::new(keymap, vial_keyboard_Id, vial_keyboard_def),
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
            vial_service
                .process_via_report(&mut usb_device.via_hid)
                .await;
            Timer::after_millis(1).await;
        }
    };
    join(usb_fut, join(keyboard_fut, via_fut)).await;

    panic!("Keyboard service is died")
}
