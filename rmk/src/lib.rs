#![doc = include_str!("../../README.md")]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]
#![allow(clippy::if_same_then_else)]

#[cfg(feature = "ble")]
use crate::ble::{keyboard_ble_task, softdevice_task};
use crate::light::LightService;
use config::{RmkConfig, VialConfig};
use core::{cell::RefCell, convert::Infallible};
use defmt::{error, warn};
use embassy_futures::select::{select4, Either4};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
pub use embedded_hal;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use futures::pin_mut;
use hid::{HidReaderWrapper, HidReaderWriterWrapper, HidWriterWrapper};
use keyboard::Keyboard;
use keymap::KeyMap;
use usb::KeyboardUsbDevice;
use via::process::VialService;

pub mod action;
#[cfg(feature = "ble")]
pub mod ble;
pub mod config;
mod debounce;
mod eeprom;
mod flash;
mod hid;
pub mod keyboard;
pub mod keycode;
pub mod keymap;
pub mod layout_macro;
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

        pin_mut!(usb_fut);
        pin_mut!(keyboard_fut);
        pin_mut!(led_reader_fut);
        pin_mut!(via_fut);

        // Run all tasks, if one of them fails, wait 1 second and then restart
        match select4(usb_fut, keyboard_fut, led_reader_fut, via_fut).await {
            Either4::First(_) => error!("Usb task is died"),
            Either4::Second(_) => error!("Keyboard task is died"),
            Either4::Third(_) => error!("Led task is died"),
            Either4::Fourth(_) => error!("Via task is died"),
        }

        warn!("Detected failure, restarting keyboard sevice after 1 second");
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
    W: HidWriterWrapper,
    W2: HidWriterWrapper,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
    keyboard_hid_writer: &mut W,
    other_hid_writer: &mut W2,
) -> ! {
    loop {
        let _ = keyboard.scan_matrix().await;
        keyboard.send_report(keyboard_hid_writer).await;
        keyboard.send_other_report(other_hid_writer).await;
    }
}

async fn led_task<R: HidReaderWrapper, Out: OutputPin>(
    keyboard_hid_reader: &mut R,
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
    Hid: HidReaderWriterWrapper,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    via_hid: &mut Hid,
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
use action::KeyAction;
#[cfg(feature = "ble")]
use embassy_executor::Spawner;
#[cfg(feature = "ble")]
pub use nrf_softdevice;
#[cfg(feature = "ble")]
/// Initialize and run the BLE keyboard service, with given keyboard usb config.
/// Can only be used on nrf52 series microcontrollers with `nrf-softdevice` crate.
/// This function never returns.
///
/// # Arguments
///
/// * `keymap` - default keymap definition
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `ble_config` - nrf_softdevice config
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `spwaner` - embassy task spwaner, used to spawn nrf_softdevice background task
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
    use embassy_futures::select::{select3, Either3};
    use heapless::FnvIndexMap;
    use nrf_softdevice::Softdevice;
    use sequential_storage::{cache::NoCache, map::fetch_item};
    use static_cell::StaticCell;
    // FIXME: add auto recognition of ble/usb
    use crate::ble::{
        advertise::{create_advertisement_data, SCAN_DATA},
        ble_battery_task,
        bonder::{BondInfo, Bonder},
        flash_task,
        server::{BleHidWriter, BleServer},
        BONDED_DEVICE_NUM, CONFIG_FLASH_RANGE,
    };
    use nrf_softdevice::{
        ble::{gatt_server, peripheral},
        Flash,
    };

    let sd = Softdevice::enable(&ble_config);
    let keyboard_name = keyboard_config
        .usb_config
        .product_name
        .unwrap_or("RMK Keyboard");
    let ble_server = unwrap!(BleServer::new(sd, keyboard_config.usb_config));
    unwrap!(spawner.spawn(softdevice_task(sd)));

    let keymap = RefCell::new(KeyMap::<F, EEPROM_SIZE, ROW, COL, NUM_LAYER>::new(
        keymap, None, None,
    ));
    let mut keyboard = Keyboard::new(input_pins, output_pins, &keymap);
    static NRF_FLASH: StaticCell<Flash> = StaticCell::new();
    let f = NRF_FLASH.init(Flash::take(sd));

    // Get all saved bond info
    let mut buf: [u8; 128] = [0; 128];

    let mut bond_info: FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM> = FnvIndexMap::new();
    for key in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(info)) =
            fetch_item::<BondInfo, _>(f, CONFIG_FLASH_RANGE, NoCache::new(), &mut buf, key as u8)
                .await
        {
            bond_info.insert(key as u8, info).ok();
        }
    }

    info!("Loaded saved bond info: {}", bond_info.len());

    // BLE bonder
    static BONDER: StaticCell<Bonder> = StaticCell::new();
    let bonder = BONDER.init(Bonder::new(RefCell::new(bond_info)));

    unwrap!(spawner.spawn(flash_task(f)));

    loop {
        info!("BLE Advertising");
        // Create connection
        let config = peripheral::Config::default();
        let adv_data = create_advertisement_data(keyboard_name);
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data,
            scan_data: &SCAN_DATA,
        };

        match peripheral::advertise_pairable(sd, adv, &config, bonder).await {
            Ok(conn) => {
                info!("Starting GATT server 1 second later");
                Timer::after_secs(1).await;
                let mut ble_writer = BleHidWriter::<'_, 8>::new(&conn, &ble_server);

                // Run the GATT server on the connection. This returns when the connection gets disconnected.
                let ble_fut = gatt_server::run(&conn, &ble_server, |_| {});
                let keyboard_fut = keyboard_ble_task(&mut keyboard, &mut ble_writer);
                let battery_fut = ble_battery_task(&ble_server, &conn);

                // Exit if anyone of three futures exits
                match select3(ble_fut, keyboard_fut, battery_fut).await {
                    Either3::First(disconnected_error) => error!(
                        "BLE gatt_server run exited with error: {:?}",
                        disconnected_error
                    ),
                    Either3::Second(_) => error!("Keyboard task exited"),
                    Either3::Third(_) => error!("Battery task exited"),
                }
            }
            Err(e) => {
                error!("Advertise error: {}", e)
            }
        }

        // Retry after 3 second
        Timer::after_secs(3).await;
    }
}
