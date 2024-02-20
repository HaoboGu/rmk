#![doc = include_str!("../../README.md")]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]

use config::{RmkConfig, VialConfig};
use core::{cell::RefCell, convert::Infallible};
use defmt::{debug, error};
use embassy_futures::join::join;
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{HidReader, HidReaderWriter, HidWriter},
    driver::Driver,
};
use embedded_hal::digital::{InputPin, OutputPin, PinState};
use embedded_storage::nor_flash::NorFlash;
use keyboard::Keyboard;
use keymap::KeyMap;
use packed_struct::PackedStructSlice;
use usb::KeyboardUsbDevice;
use via::process::VialService;

use crate::light::{LedIndicator, LightService};

pub mod action;
pub mod config;
mod debounce;
mod eeprom;
mod flash;
pub mod keyboard;
pub mod keycode;
pub mod keymap;
mod layout_macro;
mod light;
mod matrix;
mod usb;
mod via;

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
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
    keyboard_config: RmkConfig<'static>,
) -> ! {
    // TODO: Config struct for keyboard services
    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service) = (
        Keyboard::new(input_pins, output_pins, keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(keymap, keyboard_config.vial_config),
    );
    let mut light_service: LightService<Out> = LightService::new(None, None, None, PinState::Low);

    // Create 4 tasks: usb, keyboard, led, vial
    let usb_fut = usb_device.device.run();
    let keyboard_fut = keyboard_task(
        &mut keyboard,
        &mut usb_device.keyboard_hid_writer,
        &mut usb_device.other_hid_writer,
    );
    let led_reader_fut = led_task(&mut usb_device.keyboard_hid_reader, &mut light_service);
    let via_fut = vial_task(&mut usb_device.via_hid, &mut vial_service);

    // Run all tasks
    join(usb_fut, join(join(keyboard_fut, led_reader_fut), via_fut)).await;

    panic!("Keyboard service is died")
}

/// Initialize and run the keyboard service, this function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `keymap` - default keymap definition
/// * `vial_keyboard_id`/`vial_keyboard_def` - generated keyboard id and definition for vial, you can generate them automatically using [`build.rs`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/build.rs)
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
    vial_keyboard_id: &'static [u8],
    vial_keyboard_def: &'static [u8],
) -> ! {
    let mut keyboard_config = RmkConfig::default();
    keyboard_config.vial_config = VialConfig::new(vial_keyboard_id, vial_keyboard_def);

    initialize_keyboard_with_config_and_run(
        driver,
        input_pins,
        output_pins,
        keymap,
        keyboard_config,
    )
    .await
}

async fn keyboard_task<
    'a,
    D: Driver<'a>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
    keyboard_hid_writer: &mut HidWriter<'a, D, 8>,
    other_hid_writer: &mut HidWriter<'a, D, 9>,
) -> ! {
    loop {
        let _ = keyboard.keyboard_task().await;
        keyboard.send_report(keyboard_hid_writer).await;
        keyboard.send_other_report(other_hid_writer).await;
    }
}

async fn led_task<'a, D: Driver<'a>, Out: OutputPin>(
    keyboard_hid_reader: &mut HidReader<'a, D, 1>,
    light_service: &mut LightService<Out>,
) -> ! {
    let mut led_indicator_data: [u8; 1] = [0; 1];
    loop {
        match keyboard_hid_reader.read(&mut led_indicator_data).await {
            Ok(_) => {
                match LedIndicator::unpack_from_slice(&led_indicator_data) {
                    Ok(indicator) => {
                        debug!("Read keyboard state: {:?}", indicator);
                        // Ignore the result, which is `Infallible` in most cases
                        light_service.set_leds(indicator).ok();
                    }
                    Err(_) => {
                        error!("packing error: {:b}", led_indicator_data[0]);
                    }
                };
            }
            Err(e) => error!("Read keyboard state error: {}", e),
        };
        Timer::after_millis(10).await;
    }
}

async fn vial_task<
    'a,
    D: Driver<'a>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    via_hid: &mut HidReaderWriter<'a, D, 32, 32>,
    vial_service: &mut VialService<'a, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
) -> ! {
    loop {
        vial_service.process_via_report(via_hid).await;
        Timer::after_millis(1).await;
    }
}
