#![doc = include_str!("../README.md")]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]
#![allow(clippy::if_same_then_else)]
const REPORT_MAP: [u8; 65] = [
    0x05, 0x01, // USAGE_PAGE (Generic Desktop)
    0x09, 0x06, // USAGE (Keyboard)
    0xa1, 0x01, // COLLECTION (Application)
    0x85, 0x01, //   REPORT_ID (1)
    0x05, 0x07, //   USAGE_PAGE (Keyboard)
    0x19, 0x01, //   USAGE_MINIMUM
    0x29, 0x7f, //   USAGE_MAXIMUM
    0x15, 0x00, //   LOGICAL_MINIMUM (0)
    0x25, 0x01, //   LOGICAL_MAXIMUM (1)
    0x75, 0x01, //   REPORT_SIZE (1)
    0x95, 0x08, //   REPORT_COUNT (8)
    0x81, 0x02, //   INPUT (Data,Var,Abs)
    0x95, 0x01, //   REPORT_COUNT (1)
    0x75, 0x08, //   REPORT_SIZE (8)
    0x81, 0x01, //   INPUT (Cnst,Ary,Abs)
    0x95, 0x05, //   REPORT_COUNT (5)
    0x75, 0x01, //   REPORT_SIZE (1)
    0x05, 0x08, //   USAGE_PAGE (LEDs)
    0x19, 0x01, //   USAGE_MINIMUM (Num Lock)
    0x29, 0x05, //   USAGE_MAXIMUM (Kana)
    0x91, 0x02, //   OUTPUT (Data,Var,Abs)
    0x95, 0x01, //   REPORT_COUNT (1)
    0x75, 0x03, //   REPORT_SIZE (3)
    0x91, 0x01, //   OUTPUT (Cnst,Ary,Abs)
    0x95, 0x06, //   REPORT_COUNT (6)
    0x75, 0x08, //   REPORT_SIZE (8)
    0x15, 0x00, //   LOGICAL_MINIMUM (0)
    0x25, 0x65, //   LOGICAL_MAXIMUM (101)
    0x05, 0x07, //   USAGE_PAGE (Keyboard)
    0x19, 0x00, //   USAGE_MINIMUM (Reserved (no event indicated))
    0x29, 0x65, //   USAGE_MAXIMUM (Keyboard Application)
    0x81, 0x00, //   INPUT (Data,Ary,Abs)
    0xc0, // END_COLLECTION
];
// Main items
pub const HIDINPUT: u8 = 0x80;
pub const HIDOUTPUT: u8 = 0x90;
pub const FEATURE: u8 = 0xb0;
pub const COLLECTION: u8 = 0xa0;
pub const END_COLLECTION: u8 = 0xc0;

// Global items
pub const USAGE_PAGE: u8 = 0x04;
pub const LOGICAL_MINIMUM: u8 = 0x14;
pub const LOGICAL_MAXIMUM: u8 = 0x24;
pub const PHYSICAL_MINIMUM: u8 = 0x34;
pub const PHYSICAL_MAXIMUM: u8 = 0x44;
pub const UNIT_EXPONENT: u8 = 0x54;
pub const UNIT: u8 = 0x64;
pub const REPORT_SIZE: u8 = 0x74; //bits
pub const REPORT_ID: u8 = 0x84;
pub const REPORT_COUNT: u8 = 0x94; //bytes
pub const PUSH: u8 = 0xa4;
pub const POP: u8 = 0xb4;

// Local items
pub const USAGE: u8 = 0x08;
pub const USAGE_MINIMUM: u8 = 0x18;
pub const USAGE_MAXIMUM: u8 = 0x28;
pub const DESIGNATOR_INDEX: u8 = 0x38;
pub const DESIGNATOR_MINIMUM: u8 = 0x48;
pub const DESIGNATOR_MAXIMUM: u8 = 0x58;
pub const STRING_INDEX: u8 = 0x78;
pub const STRING_MINIMUM: u8 = 0x88;
pub const STRING_MAXIMUM: u8 = 0x98;
pub const DELIMITER: u8 = 0xa8;

const KEYBOARD_ID: u8 = 0x01;
const MEDIA_KEYS_ID: u8 = 0x02;

macro_rules! count {
	() => { 0u8 };
	($x:tt $($xs:tt)*) => {1u8 + count!($($xs)*)};
}

macro_rules! hid {
	($(( $($xs:tt),*)),+ $(,)?) => { &[ $( (count!($($xs)*)-1) | $($xs),* ),* ] };
}
const HID_REPORT_DESCRIPTOR: &[u8] = hid!(
    (USAGE_PAGE, 0x01), // USAGE_PAGE (Generic Desktop Ctrls)
    (USAGE, 0x06),      // USAGE (Keyboard)
    (COLLECTION, 0x01), // COLLECTION (Application)
    // ------------------------------------------------- Keyboard
    (REPORT_ID, KEYBOARD_ID), //   REPORT_ID (1)
    (USAGE_PAGE, 0x07),       //   USAGE_PAGE (Kbrd/Keypad)
    (USAGE_MINIMUM, 0xE0),    //   USAGE_MINIMUM (0xE0)
    (USAGE_MAXIMUM, 0xE7),    //   USAGE_MAXIMUM (0xE7)
    (LOGICAL_MINIMUM, 0x00),  //   LOGICAL_MINIMUM (0)
    (LOGICAL_MAXIMUM, 0x01),  //   Logical Maximum (1)
    (REPORT_SIZE, 0x01),      //   REPORT_SIZE (1)
    (REPORT_COUNT, 0x08),     //   REPORT_COUNT (8)
    (HIDINPUT, 0x02), //   INPUT (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
    (REPORT_COUNT, 0x01), //   REPORT_COUNT (1) ; 1 byte (Reserved)
    (REPORT_SIZE, 0x08), //   REPORT_SIZE (8)
    (HIDINPUT, 0x01), //   INPUT (Const,Array,Abs,No Wrap,Linear,Preferred State,No Null Position)
    (REPORT_COUNT, 0x05), //   REPORT_COUNT (5) ; 5 bits (Num lock, Caps lock, Scroll lock, Compose, Kana)
    (REPORT_SIZE, 0x01),  //   REPORT_SIZE (1)
    (USAGE_PAGE, 0x08),   //   USAGE_PAGE (LEDs)
    (USAGE_MINIMUM, 0x01), //   USAGE_MINIMUM (0x01) ; Num Lock
    (USAGE_MAXIMUM, 0x05), //   USAGE_MAXIMUM (0x05) ; Kana
    (HIDOUTPUT, 0x02), //   OUTPUT (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
    (REPORT_COUNT, 0x01), //   REPORT_COUNT (1) ; 3 bits (Padding)
    (REPORT_SIZE, 0x03), //   REPORT_SIZE (3)
    (HIDOUTPUT, 0x01), //   OUTPUT (Const,Array,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
    (REPORT_COUNT, 0x06), //   REPORT_COUNT (6) ; 6 bytes (Keys)
    (REPORT_SIZE, 0x08), //   REPORT_SIZE(8)
    (LOGICAL_MINIMUM, 0x00), //   LOGICAL_MINIMUM(0)
    (LOGICAL_MAXIMUM, 0x65), //   LOGICAL_MAXIMUM(0x65) ; 101 keys
    (USAGE_PAGE, 0x07), //   USAGE_PAGE (Kbrd/Keypad)
    (USAGE_MINIMUM, 0x00), //   USAGE_MINIMUM (0)
    (USAGE_MAXIMUM, 0x65), //   USAGE_MAXIMUM (0x65)
    (HIDINPUT, 0x00),  //   INPUT (Data,Array,Abs,No Wrap,Linear,Preferred State,No Null Position)
    (END_COLLECTION),  // END_COLLECTION
    // ------------------------------------------------- Media Keys
    (USAGE_PAGE, 0x0C),         // USAGE_PAGE (Consumer)
    (USAGE, 0x01),              // USAGE (Consumer Control)
    (COLLECTION, 0x01),         // COLLECTION (Application)
    (REPORT_ID, MEDIA_KEYS_ID), //   REPORT_ID (2)
    (USAGE_PAGE, 0x0C),         //   USAGE_PAGE (Consumer)
    (LOGICAL_MINIMUM, 0x00),    //   LOGICAL_MINIMUM (0)
    (LOGICAL_MAXIMUM, 0x01),    //   LOGICAL_MAXIMUM (1)
    (REPORT_SIZE, 0x01),        //   REPORT_SIZE (1)
    (REPORT_COUNT, 0x10),       //   REPORT_COUNT (16)
    (USAGE, 0xB5),              //   USAGE (Scan Next Track)     ; bit 0: 1
    (USAGE, 0xB6),              //   USAGE (Scan Previous Track) ; bit 1: 2
    (USAGE, 0xB7),              //   USAGE (Stop)                ; bit 2: 4
    (USAGE, 0xCD),              //   USAGE (Play/Pause)          ; bit 3: 8
    (USAGE, 0xE2),              //   USAGE (Mute)                ; bit 4: 16
    (USAGE, 0xE9),              //   USAGE (Volume Increment)    ; bit 5: 32
    (USAGE, 0xEA),              //   USAGE (Volume Decrement)    ; bit 6: 64
    (USAGE, 0x23, 0x02),        //   Usage (WWW Home)            ; bit 7: 128
    (USAGE, 0x94, 0x01),        //   Usage (My Computer) ; bit 0: 1
    (USAGE, 0x92, 0x01),        //   Usage (Calculator)  ; bit 1: 2
    (USAGE, 0x2A, 0x02),        //   Usage (WWW fav)     ; bit 2: 4
    (USAGE, 0x21, 0x02),        //   Usage (WWW search)  ; bit 3: 8
    (USAGE, 0x26, 0x02),        //   Usage (WWW stop)    ; bit 4: 16
    (USAGE, 0x24, 0x02),        //   Usage (WWW back)    ; bit 5: 32
    (USAGE, 0x83, 0x01),        //   Usage (Media sel)   ; bit 6: 64
    (USAGE, 0x8A, 0x01),        //   Usage (Mail)        ; bit 7: 128
    (HIDINPUT, 0x02), // INPUT (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
    (END_COLLECTION), // END_COLLECTION
);

use crate::{
    keyboard::keyboard_task,
    light::{led_task, LightService},
    via::vial_task,
};
use action::KeyAction;

use config::{RmkConfig, VialConfig};
use core::{cell::RefCell, convert::Infallible};
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{select, select4, Either, Either4};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
pub use embedded_hal;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use futures::pin_mut;
use keyboard::Keyboard;
use keymap::KeyMap;

use storage::Storage;
use usb::KeyboardUsbDevice;
use via::process::VialService;

pub mod action;
#[cfg(feature = "nrf_ble")]
pub mod ble;
pub mod config;
mod debounce;
mod flash;
mod hid;
pub mod keyboard;
pub mod keycode;
pub mod keymap;
pub mod layout_macro;
mod light;
mod matrix;
mod storage;
mod usb;
mod via;

/// Initialize and run the keyboard service, this function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `flash` - optional flash storage, which is used for storing keymap and keyboard configs
/// * `keymap` - default keymap definition
/// * `vial_keyboard_id`/`vial_keyboard_def` - generated keyboard id and definition for vial, you can generate them automatically using [`build.rs`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/build.rs)
pub async fn initialize_keyboard_and_run<
    D: Driver<'static>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    flash: Option<F>,
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
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
        flash,
        keymap,
        keyboard_config,
    )
    .await
}

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `flash` - optional flash storage, which is used for storing keymap and keyboard configs
/// * `keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub async fn initialize_keyboard_with_config_and_run<
    F: NorFlash,
    D: Driver<'static>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    flash: Option<F>,
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Wrap `embedded-storage` to `embedded-storage-async`
    let async_flash = flash.map(|f| embassy_embedded_hal::adapter::BlockingAsync::new(f));

    initialize_keyboard_with_config_and_run_async_flash(
        driver,
        input_pins,
        output_pins,
        async_flash,
        keymap,
        keyboard_config,
    )
    .await
}

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `flash` - optional **async** flash storage, which is used for storing keymap and keyboard configs
/// * `keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub async fn initialize_keyboard_with_config_and_run_async_flash<
    F: AsyncNorFlash,
    D: Driver<'static>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    flash: Option<F>,
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    let (mut storage, keymap) = match flash {
        Some(f) => {
            let mut s = Storage::new(f, &keymap, keyboard_config.storage_config).await;
            let keymap = RefCell::new(
                KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(keymap, Some(&mut s)).await,
            );
            (Some(s), keymap)
        }
        None => {
            let keymap = RefCell::new(
                KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage::<F>(keymap, None).await,
            );
            (None, keymap)
        }
    };

    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(input_pins, output_pins, &keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    loop {
        // Run all tasks, if one of them fails, wait 1 second and then restart
        if let Some(ref mut s) = storage {
            run_usb_keyboard(
                &mut usb_device,
                &mut keyboard,
                s,
                &mut light_service,
                &mut vial_service,
            )
            .await;
        } else {
            // Run 4 tasks: usb, keyboard, led, vial
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
            match select4(usb_fut, keyboard_fut, led_reader_fut, via_fut).await {
                Either4::First(_) => {
                    error!("Usb task is died");
                }
                Either4::Second(_) => error!("Keyboard task is died"),
                Either4::Third(_) => error!("Led task is died"),
                Either4::Fourth(_) => error!("Via task is died"),
            }
        }

        warn!("Detected failure, restarting keyboard sevice after 1 second");
        Timer::after_secs(1).await;
    }
}

#[cfg(feature = "nrf_ble")]
use embassy_executor::Spawner;
#[cfg(feature = "nrf_ble")]
pub use nrf_softdevice;
#[cfg(feature = "nrf_ble")]
use nrf_softdevice::ble::{gatt_server, Connection};
#[cfg(feature = "nrf_ble")]
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
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `spwaner` - embassy task spwaner, used to spawn nrf_softdevice background task
pub async fn initialize_nrf_ble_keyboard_with_config_and_run<
    #[cfg(not(feature = "nrf52832_ble"))] D: Driver<'static>,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    #[cfg(not(feature = "nrf52832_ble"))] usb_driver: Option<D>,
    keyboard_config: RmkConfig<'static, Out>,
    spawner: Spawner,
) -> ! {
    #[cfg(not(feature = "nrf52832_ble"))]
    use crate::usb::{wait_for_usb_configured, wait_for_usb_suspend, USB_DEVICE_ENABLED};
    use crate::{
        ble::{
            advertise::{create_advertisement_data, SCAN_DATA},
            bonder::{BondInfo, Bonder},
            nrf_ble_config, softdevice_task, BONDED_DEVICE_NUM,
        },
        storage::{get_bond_info_key, StorageData},
    };
    #[cfg(not(feature = "nrf52832_ble"))]
    use core::sync::atomic::Ordering;
    use defmt::*;
    use heapless::FnvIndexMap;
    use nrf_softdevice::{ble::peripheral, Flash, Softdevice};
    use sequential_storage::{cache::NoCache, map::fetch_item};
    use static_cell::StaticCell;

    // Set ble config and start nrf-softdevice background task first
    let keyboard_name = keyboard_config
        .usb_config
        .product_name
        .unwrap_or("RMK Keyboard");
    let ble_config = nrf_ble_config(keyboard_name);

    let sd = Softdevice::enable(&ble_config);
    let ble_server = unwrap!(BleServer::new(sd, keyboard_config.usb_config));
    unwrap!(spawner.spawn(softdevice_task(sd)));

    // Flash and keymap configuration
    let flash = Flash::take(sd);
    let mut storage = Storage::new(flash, &keymap, keyboard_config.storage_config).await;
    let keymap = RefCell::new(
        KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(keymap, Some(&mut storage)).await,
    );

    // Get all saved bond info, config BLE bonder
    let mut buf: [u8; 128] = [0; 128];
    let mut bond_info: FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM> = FnvIndexMap::new();
    for key in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(StorageData::BondInfo(info))) =
            fetch_item::<StorageData<ROW, COL, NUM_LAYER>, _>(
                &mut storage.flash,
                storage.storage_range.clone(),
                NoCache::new(),
                &mut buf,
                get_bond_info_key(key as u8),
            )
            .await
        {
            bond_info.insert(key as u8, info).ok();
        }
    }
    info!("Loaded saved bond info: {}", bond_info.len());
    static BONDER: StaticCell<Bonder> = StaticCell::new();
    let bonder = BONDER.init(Bonder::new(RefCell::new(bond_info)));

    // Keyboard services
    let mut keyboard = Keyboard::new(input_pins, output_pins, &keymap);
    #[cfg(not(feature = "nrf52832_ble"))]
    let (mut usb_device, mut vial_service, mut light_service) = (
        usb_driver.map(|u| KeyboardUsbDevice::new(u, keyboard_config.usb_config)),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    // Main loop
    loop {
        // Init BLE advertising data
        let config = peripheral::Config::default();
        let adv_data = create_advertisement_data(keyboard_name);
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data,
            scan_data: &SCAN_DATA,
        };
        let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);

        // If there is a USB device, things become a little bit complex because we need to enable switching between USB and BLE.
        // Remember that USB ALWAYS has higher priority than BLE.
        //
        // If no USB device, just start BLE advertising
        #[cfg(not(feature = "nrf52832_ble"))]
        if let Some(ref mut usb_device) = usb_device {
            // Check and run via USB first
            if USB_DEVICE_ENABLED.load(Ordering::SeqCst) {
                let usb_fut = run_usb_keyboard(
                    usb_device,
                    &mut keyboard,
                    &mut storage,
                    &mut light_service,
                    &mut vial_service,
                );
                select(usb_fut, wait_for_usb_suspend()).await;
            }

            // Usb device have to be started to check if usb is configured
            let usb_fut = usb_device.device.run();
            let usb_configured = wait_for_usb_configured();
            info!("USB suspended, BLE Advertising");

            // Wait for BLE or USB connection
            match select(adv_fut, select(usb_fut, usb_configured)).await {
                Either::First(re) => match re {
                    Ok(conn) => {
                        info!("Connected to BLE");
                        let usb_configured = wait_for_usb_configured();
                        let usb_fut = usb_device.device.run();
                        match select(
                            run_ble_keyboard(&conn, &ble_server, &mut keyboard, &mut storage),
                            select(usb_fut, usb_configured),
                        )
                        .await
                        {
                            Either::First(_) => (),
                            Either::Second(_) => {
                                info!("Detected USB configured, quit BLE");
                                continue;
                            }
                        }
                    }
                    Err(e) => error!("Advertise error: {}", e),
                },
                Either::Second(_) => {
                    // Wait 10ms for usb resuming
                    Timer::after_millis(10).await;
                    continue;
                }
            }
        } else {
            match adv_fut.await {
                Ok(conn) => run_ble_keyboard(&conn, &ble_server, &mut keyboard, &mut storage).await,
                Err(e) => error!("Advertise error: {}", e),
            }
        }

        #[cfg(feature = "nrf52832_ble")]
        match adv_fut.await {
            Ok(conn) => run_ble_keyboard(&conn, &ble_server, &mut keyboard, &mut storage).await,
            Err(e) => error!("Advertise error: {}", e),
        }
        // Retry after 3 second
        Timer::after_millis(100).await;
    }
}

// BLE HID readers writers for esp
fn read_hid_info(_offset: usize, data: &mut [u8]) -> usize {
    data[0..4].copy_from_slice(&[
        0x1u8, 0x1u8,  // HID version: 1.1
        0x00u8, // Country Code
        0x03u8, // Remote wake + Normally Connectable
    ]);
    4
}
fn read_hid_report_map(_offset: usize, data: &mut [u8]) -> usize {
    data[0..HID_REPORT_DESCRIPTOR.len()].copy_from_slice(HID_REPORT_DESCRIPTOR);
    HID_REPORT_DESCRIPTOR.len()
}

fn write_hid_control_point(offset: usize, data: &[u8]) {
    println!("write hid control point: {} {:?}", offset, data);
}

fn write_protocol_mode(offset: usize, data: &[u8]) {
    println!("write_protocol_mode: Offset {}, data {:?}", offset, data);
}
fn read_protocol_mode(_offset: usize, data: &mut [u8]) -> usize {
    data[0] = 0;
    data[1] = 0;
    2
}
fn read_hid_report(_offset: usize, data: &mut [u8]) -> usize {
    data[0..8].copy_from_slice(&[0, 0, 0x04, 0, 0, 0, 0, 0]);
    8
}

fn read_device_info(_offset: usize, data: &mut [u8]) -> usize {
    data[..7].copy_from_slice(&[0x02, 0x4c, 0x4b, 0x46, 0x43, 0x00, 0x00]);
    7
}

fn read_battery_level(_offset: usize, data: &mut [u8]) -> usize {
    data[..1].copy_from_slice(&[80]);
    1
}

use esp_hal::Rng;
struct RngWrapper {
    rng: Rng,
}

impl rand_core::RngCore for RngWrapper {
    fn next_u32(&mut self) -> u32 {
        self.rng.random()
    }

    fn next_u64(&mut self) -> u64 {
        (self.rng.random() as u64) << 32 | self.rng.random() as u64
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for b in dest {
            *b = (self.rng.random() & 0xff) as u8;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl rand_core::CryptoRng for RngWrapper {}

use bleps::{
    ad_structure::{
        create_advertising_data, AdStructure, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
    },
    async_attribute_server::AttributeServer,
    asynch::Ble,
    attribute_server::{NotificationData, WorkResult},
    gatt, Addr,
};
pub async fn initialize_esp_ble_keyboard_with_config_and_run<
    T: embedded_io_async::Read + embedded_io_async::Write,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    keyboard_config: RmkConfig<'static, Out>,
    ble: &mut Ble<T>,
    rng: Rng,
) -> ! {
    let keymap = RefCell::new(KeyMap::<ROW, COL, NUM_LAYER>::new(keymap).await);
    let mut rng_wrap = RngWrapper { rng };

    let mut keyboard = Keyboard::new(input_pins, output_pins, &keymap);

    loop {
        ble.init().await.unwrap();
        ble.cmd_set_le_advertising_parameters().await.unwrap();
        ble.cmd_set_le_advertising_data(
            create_advertising_data(&[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::Unknown {
                    ty: 0x03,
                    data: &[0x12, 0x18],
                }, // HID
                AdStructure::CompleteLocalName("ESP RMK Keyboard"),
                AdStructure::Unknown {
                    ty: 0x19,
                    data: &[0xC1, 0x03],
                }, // Appearance
            ])
            .unwrap(),
        )
        .await
        .unwrap();

        ble.cmd_set_le_advertise_enable(true).await.unwrap();
        info!("Start advertising");
        let mut read_hid_report_map2 = |offset: usize, data: &mut [u8]| {
            // println!("read hid report map {offset} {}", data.len());

            let val = REPORT_MAP;
            let off = offset;
            if off < val.len() {
                let len = data.len().min(val.len() - off);
                data[..len].copy_from_slice(&val[off..off + len]);
                len
            } else {
                0
            }
        };
        let local_addr = Addr::from_le_bytes(false, ble.cmd_read_br_addr().await.unwrap());
        let mut ltk = None;

        gatt!([
            // BLE HID Service
            service {
                uuid: "00001812-0000-1000-8000-00805f9b34fb",
                characteristics: [
                    // BLE HID_information
                    characteristic {
                        uuid: "00002a4a-0000-1000-8000-00805f9b34fb",
                        read: read_hid_info,
                    },
                    // BLE HID Report Map characteristic
                    characteristic {
                        uuid: "00002a4b-0000-1000-8000-00805f9b34fb",
                        read: read_hid_report_map2,
                    },
                    // BLE HID control point characteristic
                    characteristic {
                        uuid: "00002a4c-0000-1000-8000-00805f9b34fb",
                        write: write_hid_control_point,
                    },
                    // BLE HID Report characteristic
                    characteristic {
                        uuid: "00002a4d-0000-1000-8000-00805f9b34fb",
                        name: "hid_report",
                        notify: true,
                        read: read_hid_report,
                    },
                    // BLE HID protocol mode characteristic
                    characteristic {
                        uuid: "00002a4e-0000-1000-8000-00805f9b34fb",
                        write: write_protocol_mode,
                        read: read_protocol_mode,
                    },
                ],
            },
            // BLE device information
            service {
                uuid: "0000180a-0000-1000-8000-00805f9b34fb",
                characteristics: [
                    // BLE Device Information characteristic
                    characteristic {
                        uuid: "00002a50-0000-1000-8000-00805f9b34fb",
                        read: read_device_info,
                    },
                ],
            },
            // BLE HID Battery Service
            service {
                uuid: "0000180f-0000-1000-8000-00805f9b34fb",
                // BLE HID battery level characteristic
                characteristics: [characteristic {
                    uuid: "00002a19-0000-1000-8000-00805f9b34fb",
                    read: read_battery_level,
                },],
            },
        ]);
        // Create service
        // gatt!([
        //     // HID service
        //     service {
        //         uuid: "1812",
        //         characteristics: [
        //             characteristic {
        //                 uuid: "2A4A",
        //                 read: read_hid_info,
        //             },
        //             characteristic {
        //                 uuid: "2A4B",
        //                 read: read_hid_report_map,
        //             },
        //             characteristic {
        //                 uuid: "2A4C",
        //                 write: write_hid_control_point,
        //             },
        //             characteristic {
        //                 uuid: "2A4E",
        //                 read: read_protocol_mode,
        //                 write: write_protocol_mode,
        //             },
        //             characteristic {
        //                 uuid: "2A4D",
        //                 notify: true,
        //                 name: "hid_report",
        //                 read: read_hid_report,
        //             },
        //         ],
        //     },
        //     // BLE device information
        //     service {
        //         uuid: "180A",
        //         characteristics: [
        //             // BLE Device Information characteristic
        //             characteristic {
        //                 uuid: "2a50",
        //                 read: read_device_info,
        //             },
        //         ],
        //     },
        //     // BLE HID Battery Service
        //     service {
        //         uuid: "180F",
        //         // BLE HID battery level characteristic
        //         characteristics: [characteristic {
        //             uuid: "2a19",
        //             read: read_battery_level,
        //         },],
        //     },
        // ]);
        let mut srv = AttributeServer::new_with_ltk(
            ble,
            &mut gatt_attributes,
            local_addr,
            ltk,
            &mut rng_wrap,
        );
        let mut notifier = || async {
            Timer::after_secs(2).await;
            println!("notify hid report");

            NotificationData::new(hid_report_handle, &[1, 0, 0, 4, 0, 0, 0, 0, 0])
        };
        srv.run(&mut notifier).await.unwrap();
        // loop {
        //     info!("into loop");
        //     let mut notification = Some(NotificationData::new(
        //         hid_report_handle,
        //         &[1, 0, 0, 4, 0, 0, 0, 0, 0],
        //     ));
        //     let mut cccd = [0u8; 1];
        //     if let Some(1) =
        //         srv.get_characteristic_value(hid_report_notify_enable_handle, 0, &mut cccd)
        //     {
        //         // if notifications enabled
        //         if cccd[0] == 1 {
        //             notification = Some(NotificationData::new(
        //                 hid_report_handle,
        //                 &b"Notification"[..],
        //             ));
        //         }
        //     }

        //     match srv.do_work_with_notification(notification).await {
        //         Ok(res) => {
        //             if let WorkResult::GotDisconnected = res {
        //                 ltk = srv.get_ltk();
        //                 info!("disconnected, got ltk {}", ltk);
        //             } else {
        //                 ltk = srv.get_ltk();
        //                 info!("did work, got ltk {}", ltk);
        //             }
        //         }
        //         Err(err) => {
        //             info!("error: {:?}", Debug2Format(&err));
        //         }
        //     }

        //     Timer::after_millis(1000).await;
        // }
    }
}

// Run usb keyboard task for once
async fn run_usb_keyboard<
    'a,
    'b,
    D: Driver<'a>,
    F: AsyncNorFlash,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    usb_device: &mut KeyboardUsbDevice<'a, D>,
    keyboard: &mut Keyboard<'b, In, Out, ROW, COL, NUM_LAYER>,
    storage: &mut Storage<F>,
    light_service: &mut LightService<Out>,
    vial_service: &mut VialService<'b, ROW, COL, NUM_LAYER>,
) {
    let usb_fut = usb_device.device.run();
    let keyboard_fut = keyboard_task(
        keyboard,
        &mut usb_device.keyboard_hid_writer,
        &mut usb_device.other_hid_writer,
    );
    let led_reader_fut = led_task(&mut usb_device.keyboard_hid_reader, light_service);
    let via_fut = vial_task(&mut usb_device.via_hid, vial_service);
    let storage_fut = storage.run::<ROW, COL, NUM_LAYER>();
    pin_mut!(usb_fut);
    pin_mut!(keyboard_fut);
    pin_mut!(led_reader_fut);
    pin_mut!(via_fut);
    pin_mut!(storage_fut);
    match select4(
        select(usb_fut, keyboard_fut),
        storage_fut,
        led_reader_fut,
        via_fut,
    )
    .await
    {
        Either4::First(e) => match e {
            Either::First(_) => error!("Usb task is died"),
            Either::Second(_) => error!("Keyboard task is died"),
        },
        Either4::Second(_) => error!("Storage task is died"),
        Either4::Third(_) => error!("Led task is died"),
        Either4::Fourth(_) => error!("Via task is died"),
    }
}

#[cfg(feature = "nrf_ble")]
use crate::ble::server::BleServer;
#[cfg(feature = "nrf_ble")]
// Run ble keyboard task for once
async fn run_ble_keyboard<
    'a,
    F: AsyncNorFlash,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    conn: &Connection,
    ble_server: &BleServer,
    keyboard: &mut Keyboard<'a, In, Out, ROW, COL, NUM_LAYER>,
    storage: &mut Storage<F>,
) {
    use crate::ble::{ble_battery_task, keyboard_ble_task, server::BleHidWriter};

    info!("Starting GATT server 200 ms later");
    Timer::after_millis(200).await;
    let mut ble_keyboard_writer = BleHidWriter::<'_, 8>::new(&conn, ble_server.hid.input_keyboard);
    let mut ble_media_writer = BleHidWriter::<'_, 2>::new(&conn, ble_server.hid.input_media_keys);
    let mut ble_system_control_writer =
        BleHidWriter::<'_, 1>::new(&conn, ble_server.hid.input_system_keys);
    let mut ble_mouse_writer = BleHidWriter::<'_, 5>::new(&conn, ble_server.hid.input_mouse_keys);

    // Run the GATT server on the connection. This returns when the connection gets disconnected.
    let ble_fut = gatt_server::run(&conn, ble_server, |_| {});
    let keyboard_fut = keyboard_ble_task(
        keyboard,
        &mut ble_keyboard_writer,
        &mut ble_media_writer,
        &mut ble_system_control_writer,
        &mut ble_mouse_writer,
    );
    let battery_fut = ble_battery_task(&ble_server, &conn);
    let storage_fut = storage.run::<ROW, COL, NUM_LAYER>();

    // Exit if anyone of three futures exits
    match select4(ble_fut, keyboard_fut, battery_fut, storage_fut).await {
        Either4::First(disconnected_error) => error!(
            "BLE gatt_server run exited with error: {:?}",
            disconnected_error
        ),
        Either4::Second(_) => error!("Keyboard task exited"),
        Either4::Third(_) => error!("Battery task exited"),
        Either4::Fourth(_) => error!("Storage task exited"),
    }
}
