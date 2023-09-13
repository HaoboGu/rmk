#![no_std]
#![feature(type_alias_impl_trait)]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]

use action::KeyAction;
use config::KeyboardConfig;
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use keyboard::Keyboard;
use usb::KeyboardUsbDevice;
use usb_device::class_prelude::{UsbBus, UsbBusAllocator};

pub mod action;
pub mod config;
pub mod debounce;
pub mod keyboard;
pub mod keycode;
pub mod keymap;
pub mod layout_macro;
pub mod matrix;
pub mod usb;

/// Initialize keyboard core and keyboard usb device
pub fn initialize_keyboard_and_usb_device<
    'a,
    B: UsbBus,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    usb_allocator: &'a UsbBusAllocator<B>,
    config: &KeyboardConfig<'a>,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
) -> (
    Keyboard<In, Out, ROW, COL, NUM_LAYER>,
    KeyboardUsbDevice<'a, B>,
) {
    (
        Keyboard::new(input_pins, output_pins, keymap),
        KeyboardUsbDevice::new(usb_allocator, config),
    )
}
