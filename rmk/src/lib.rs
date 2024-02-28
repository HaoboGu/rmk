#![doc = include_str!("../../README.md")]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]
#![allow(clippy::if_same_then_else)]

use crate::light::LightService;
use action::KeyAction;
use config::{RmkConfig, VialConfig};
use core::{cell::RefCell, convert::Infallible};
use defmt::error;
use embassy_futures::join::join;
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{HidReader, HidReaderWriter, HidWriter},
    driver::Driver,
};
pub use embedded_hal::digital::{InputPin, OutputPin, PinState};
use embedded_storage::nor_flash::NorFlash;
use keyboard::Keyboard;
use keymap::KeyMap;
#[cfg(feature = "ble")]
pub use nrf_softdevice;
use usb::KeyboardUsbDevice;
use via::process::VialService;

pub mod action;
#[cfg(feature = "ble")]
pub mod ble;
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
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // TODO: Config struct for keyboard services
    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service) = (
        Keyboard::new(input_pins, output_pins, keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(keymap, keyboard_config.vial_config),
    );

    let mut light_service: LightService<Out> =
        LightService::from_config(keyboard_config.light_config);

    loop {
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

        error!("Keyboard service is died");
        Timer::after_secs(1).await;
    }
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
    let keyboard_config = RmkConfig {
        vial_config: VialConfig::new(vial_keyboard_id, vial_keyboard_def),
        ..Default::default()
    };

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
    loop {
        match light_service.check_led_indicator(keyboard_hid_reader).await {
            Ok(_) => Timer::after_millis(50).await,
            Err(_) => Timer::after_secs(2).await,
        }
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
        match vial_service.process_via_report(via_hid).await {
            Ok(_) => Timer::after_millis(1).await,
            Err(_) => Timer::after_millis(500).await,
        }
    }
}

#[cfg(feature = "ble")]
#[embassy_executor::task]
async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    sd.run().await
}

#[cfg(feature = "ble")]
use crate::ble::BleServer;
use embassy_executor::Spawner;
#[cfg(feature = "ble")]
/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub async fn initialize_ble_keyboard_with_config_and_run<
    F: NorFlash,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    ble_config: nrf_softdevice::Config,
    keyboard_config: RmkConfig<'static, Out>,
    spawner: Spawner,
) -> ! {
    use defmt::*;
    use nrf_softdevice::Softdevice;
    use static_cell::StaticCell;
    // FIXME: add auto recognition of ble/usb
    use crate::ble::{
        constants::{ADV_DATA, SCAN_DATA},
        Bonder,
    };
    use nrf_softdevice::ble::{gatt_server, peripheral};
    let sd = Softdevice::enable(&ble_config);
    let ble_server = unwrap!(BleServer::new(sd, keyboard_config.usb_config));
    unwrap!(spawner.spawn(softdevice_task(sd)));

    let keymap = RefCell::new(KeyMap::<F, EEPROM_SIZE, ROW, COL, NUM_LAYER>::new(
        keymap, None, None,
    ));
    let mut keyboard = Keyboard::new(input_pins, output_pins, &keymap);

    static BONDER: StaticCell<Bonder> = StaticCell::new();
    let bonder = BONDER.init(Bonder::default());

    loop {
        info!("Advertising");
        // Create connection
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &ADV_DATA,
            scan_data: &SCAN_DATA,
        };

        match peripheral::advertise_pairable(sd, adv, &config, bonder).await {
            Ok(conn) => {
                info!("Starting GATT server");
                // Run the GATT server on the connection. This returns when the connection gets disconnected.
                // FIXME: add keyboard task, add ble send to keyboard send_hid_report
                let ble_fut = gatt_server::run(&conn, &ble_server, |_| {});
                let keyboard_fut = keyboard_ble_task(&mut keyboard, &ble_server, &conn);
                let (disconnected_error, _) = join(ble_fut, keyboard_fut).await;

                error!(
                    "BLE gatt_server run exited with error: {:?}",
                    disconnected_error
                );
            }
            Err(e) => {
                error!("Advertise error: {}", e)
            }
        }

        // Retry after 1 second
        Timer::after_secs(1).await;
    }
}

async fn keyboard_ble_task<
    'a,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
    ble_server: &BleServer,
    conn: &nrf_softdevice::ble::Connection,
) {
    loop {
        Timer::after_secs(5).await;
        // FIXME: Send report only after all connections are ready, otherwise a BleGattsSysAttrMissing would occur
        let _ = keyboard.keyboard_task().await;
        ble_server.hid.send_keyboard_report(
            conn,
            &[
                0, // Modifiers (Shift, Ctrl, Alt, GUI, etc.)
                0, // Reserved
                0x04, 0x00, 0, 0, 0,
                0, // Key code array - 0x04 is 'a' and 0x1d is 'z' - for example
            ],
        );
    }
}
