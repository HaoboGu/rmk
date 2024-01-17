#![doc = include_str!("../../README.md")]
#![feature(type_alias_impl_trait)]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]

use action::KeyAction;
use core::convert::Infallible;
use eeprom::{eeconfig::Eeconfig, EepromStorageConfig};
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use keyboard::Keyboard;
use usb::KeyboardUsbDevice;

pub use embassy_sync;
pub use embassy_usb;
pub use usbd_hid;

pub mod action;
pub mod config;
pub mod debounce;
pub mod eeprom;
pub mod flash;
pub mod keyboard;
pub mod keycode;
pub mod keymap;
pub mod layout_macro;
pub mod matrix;
pub mod usb;
pub mod via;

// DEPRECIATED
// pub fn initialize_keyboard_and_usb_device<
//     'a,
//     B: UsbBus,
//     In: InputPin<Error = Infallible>,
//     Out: OutputPin<Error = Infallible>,
//     F: NorFlash,
//     const EEPROM_SIZE: usize,
//     const ROW: usize,
//     const COL: usize,
//     const NUM_LAYER: usize,
// >(
//     usb_allocator: &'a UsbBusAllocator<B>,
//     config: &KeyboardConfig<'a>,
//     storage: Option<F>,
//     eeprom_storage_config: EepromStorageConfig,
//     eeconfig: Option<Eeconfig>,
//     input_pins: [In; ROW],
//     output_pins: [Out; COL],
//     keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
// ) -> (
//     Keyboard<In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
//     KeyboardUsbDevice<'a, B>,
// ) {
//     (
//         Keyboard::new(
//             input_pins,
//             output_pins,
//             storage,
//             eeprom_storage_config,
//             eeconfig,
//             keymap,
//         ),
//         KeyboardUsbDevice::new(usb_allocator, config),
//     )
// }

/// Initialize keyboard core and keyboard usb device
pub fn initialize_keyboard_and_usb_device<
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
    storage: Option<F>,
    eeprom_storage_config: EepromStorageConfig,
    eeconfig: Option<Eeconfig>,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
) -> (
    Keyboard<In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
    KeyboardUsbDevice<'static, D>,
) {
    (
        Keyboard::new(
            input_pins,
            output_pins,
            storage,
            eeprom_storage_config,
            eeconfig,
            keymap,
        ),
        KeyboardUsbDevice::new(driver),
    )
}
