#![doc = include_str!("../README.md")]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std in test
#![cfg_attr(not(test), no_std)]
#![allow(clippy::if_same_then_else)]

use crate::{
    keyboard::keyboard_task,
    light::{led_task, LightService},
    via::vial_task,
};
use action::KeyAction;
use config::{RmkConfig, VialConfig};
use core::{cell::RefCell, convert::Infallible};
use defmt::*;
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
#[cfg(feature = "ble")]
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
            let mut s = Storage::new(f, &keymap).await;
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

#[cfg(feature = "ble")]
use embassy_executor::Spawner;
#[cfg(feature = "ble")]
pub use nrf_softdevice;
#[cfg(feature = "ble")]
use nrf_softdevice::ble::{gatt_server, Connection};
#[cfg(feature = "ble")]
#[doc(cfg(feature = "ble"))]
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
    let mut storage = Storage::new(flash, &keymap).await;
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

#[cfg(feature = "ble")]
use crate::ble::server::BleServer;
#[cfg(feature = "ble")]
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
