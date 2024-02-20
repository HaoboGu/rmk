#![doc = include_str!("../../README.md")]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]

use config::{RmkConfig, VialConfig};
use core::{cell::RefCell, convert::Infallible};
use defmt::{error, info};
use embassy_futures::join::join;
use embassy_time::Timer;
use embassy_usb::driver::Driver;
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
    let (mut keyboard, mut usb_device, vial_service) = (
        Keyboard::new(input_pins, output_pins, keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(keymap, keyboard_config.vial_config),
    );

    // TODO: Config struct for keyboard light service
    let mut light_service: LightService<Out> = LightService::new(None, None, None, PinState::Low);

    let usb_fut = usb_device.device.run();
    let keyboard_fut = async {
        loop {
            let _ = keyboard.keyboard_task().await;
            keyboard
                .send_report(&mut usb_device.keyboard_hid_writer)
                .await;
            keyboard.send_other_report(&mut usb_device.other_hid).await;
        }
    };

    // Keyboard state defined in protocol, aka capslock/numslock/scrolllock
    let led_reader_fut = async {
        let mut led_indicator_data: [u8; 1] = [0; 1];
        loop {
            match usb_device
                .keyboard_hid_reader
                .read(&mut led_indicator_data)
                .await
            {
                Ok(_) => {
                    let indicator = match LedIndicator::unpack_from_slice(&led_indicator_data) {
                        Ok(p) => p,
                        Err(_) => {
                            info!("packing error: {:b}", led_indicator_data[0]);
                            LedIndicator::default()
                        }
                    };
                    info!("Read keyboard state: {:?}", indicator);
                    // Ignore the result, which is `Infallible` in most cases
                    light_service.set_leds(indicator).ok();
                }
                Err(e) => error!("Read keyboard state error: {}", e),
            };
            Timer::after_millis(10).await;
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
