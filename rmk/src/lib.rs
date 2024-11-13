#![doc = include_str!("../README.md")]
//! ## Feature flags
#![doc = document_features::document_features!()]
// Add docs.rs logo
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/HaoboGu/rmk/23d7e5009a7ba28bdab13d892c5aec53a6a94703/docs/src/images/rmk_logo.png"
)]
// Make compiler and rust analyzer happy
#![allow(dead_code)]
#![allow(non_snake_case, non_upper_case_globals)]
// Enable std for espidf and test
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "_esp_ble")]
use crate::ble::esp::initialize_esp_ble_keyboard_with_config_and_run;
#[cfg(feature = "_nrf_ble")]
use crate::ble::nrf::initialize_nrf_ble_keyboard_and_run;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::{
    light::{led_hid_task, LightService},
    via::vial_task,
};
use action::KeyAction;
use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU8},
};
use debounce::DebouncerTrait;
use defmt::{error, warn};
#[cfg(not(feature = "_esp_ble"))]
use embassy_executor::Spawner;
use embassy_futures::select::{select, select4, Either4};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
pub use embedded_hal;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
#[cfg(not(feature = "_no_external_storage"))]
use embedded_storage::nor_flash::NorFlash;
pub use flash::EmptyFlashWrapper;
use futures::pin_mut;
use keyboard::{
    communication_task, keyboard_report_channel, Keyboard, KeyboardReportMessage,
    REPORT_CHANNEL_SIZE,
};
use keymap::KeyMap;
use matrix::{Matrix, MatrixTrait};
pub use rmk_config as config;
use rmk_config::RmkConfig;
pub use rmk_macro as macros;
use usb::KeyboardUsbDevice;
use via::process::VialService;
#[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
use {embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash, storage::Storage};

pub mod action;
#[cfg(feature = "_ble")]
pub mod ble;
mod debounce;
pub mod direct_pin;
mod flash;
mod hid;
mod keyboard;
mod keyboard_macro;
pub mod keycode;
mod keymap;
mod layout_macro;
mod light;
mod matrix;
#[cfg(feature = "split")]
pub mod split;
mod storage;
mod usb;
mod via;

/// Keyboard state, true for started, false for stopped
pub(crate) static KEYBOARD_STATE: AtomicBool = AtomicBool::new(false);
/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);

/// Run RMK keyboard service. This function should never return.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `output_pins` - output gpio pins
/// * `usb_driver` - (optional) embassy usb driver instance. Some microcontrollers would enable the `_no_usb` feature implicitly, which eliminates this argument
/// * `flash` - (optional) flash storage, which is used for storing keymap and keyboard configs. Some microcontrollers would enable the `_no_external_storage` feature implicitly, which eliminates this argument
/// * `default_keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
pub async fn run_rmk<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(not(feature = "_no_external_storage"))] F: NorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(not(feature = "_no_external_storage"))] flash: F,
    default_keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    // Wrap `embedded-storage` to `embedded-storage-async`
    #[cfg(not(feature = "_no_external_storage"))]
    let async_flash = embassy_embedded_hal::adapter::BlockingAsync::new(flash);

    run_rmk_with_async_flash(
        input_pins,
        output_pins,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        #[cfg(not(feature = "_no_external_storage"))]
        async_flash,
        default_keymap,
        keyboard_config,
        #[cfg(not(feature = "_esp_ble"))]
        spawner,
    )
    .await
}

/// Run RMK keyboard service. This function should never return.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `output_pins` - output gpio pins
/// * `usb_driver` - (optional) embassy usb driver instance. Some microcontrollers would enable the `_no_usb` feature implicitly, which eliminates this argument
/// * `flash` - (optional) async flash storage, which is used for storing keymap and keyboard configs. Some microcontrollers would enable the `_no_external_storage` feature implicitly, which eliminates this argument
/// * `default_keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
#[allow(unused_variables)]
#[allow(unreachable_code)]
pub async fn run_rmk_with_async_flash<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(not(feature = "_no_external_storage"))] F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(not(feature = "_no_external_storage"))] flash: F,
    default_keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    // Create the debouncer, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let debouncer = RapidDebouncer::<ROW, COL>::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer = RapidDebouncer::<COL, ROW>::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer = DefaultDebouncer::<COL, ROW>::new();

    // Keyboard matrix, use COL2ROW by default
    #[cfg(feature = "col2row")]
    let matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let matrix = Matrix::<_, _, _, COL, ROW>::new(input_pins, output_pins, debouncer);

    // Dispatch according to chip and communication type
    #[cfg(feature = "_nrf_ble")]
    initialize_nrf_ble_keyboard_and_run(
        matrix,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        default_keymap,
        keyboard_config,
        None,
        spawner,
    )
    .await;

    #[cfg(feature = "_esp_ble")]
    initialize_esp_ble_keyboard_with_config_and_run(matrix, default_keymap, keyboard_config).await;

    #[cfg(all(
        not(feature = "_no_usb"),
        not(any(feature = "_nrf_ble", feature = "_esp_ble"))
    ))]
    initialize_usb_keyboard_and_run(
        matrix,
        usb_driver,
        #[cfg(not(feature = "_no_external_storage"))]
        flash,
        default_keymap,
        keyboard_config,
    )
    .await;

    // The fut should never return.
    // If there's no fut, the feature flags must not be correct.
    defmt::panic!("The run_rmk should never return");
}

pub(crate) async fn initialize_usb_keyboard_and_run<
    Out: OutputPin,
    D: Driver<'static>,
    M: MatrixTrait,
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))] F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    mut matrix: M,
    usb_driver: D,
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))] flash: F,
    default_keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    // For USB keyboard, the "external" storage means the storage initialized by the user.
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
    let (mut storage, keymap) = {
        let mut s = Storage::new(flash, default_keymap, keyboard_config.storage_config).await;
        let keymap = RefCell::new(KeyMap::new_from_storage(default_keymap, Some(&mut s)).await);
        (s, keymap)
    };
    #[cfg(all(not(feature = "_nrf_ble"), feature = "_no_external_storage"))]
    let keymap = RefCell::new(KeyMap::<ROW, COL, NUM_LAYER>::new(default_keymap).await);

    let keyboard_report_sender = keyboard_report_channel.sender();
    let keyboard_report_receiver = keyboard_report_channel.receiver();

    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(
            &keymap,
            &keyboard_report_sender,
            keyboard_config.keyboard_options_config,
        ),
        KeyboardUsbDevice::new(usb_driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    KEYBOARD_STATE.store(false, core::sync::atomic::Ordering::Release);
    // Run all tasks, if one of them fails, wait 1 second and then restart
    run_usb_keyboard(
        &mut usb_device,
        &mut keyboard,
        &mut matrix,
        #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
        &mut storage,
        &mut light_service,
        &mut vial_service,
        &keyboard_report_receiver,
    )
    .await
}

// Run usb keyboard task for once
pub(crate) async fn run_usb_keyboard<
    'a,
    'b,
    D: Driver<'a>,
    M: MatrixTrait,
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))] F: AsyncNorFlash,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    usb_device: &mut KeyboardUsbDevice<'a, D>,
    keyboard: &mut Keyboard<'b, ROW, COL, NUM_LAYER>,
    matrix: &mut M,
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))] storage: &mut Storage<
        F,
        ROW,
        COL,
        NUM_LAYER,
    >,
    light_service: &mut LightService<Out>,
    vial_service: &mut VialService<'b, ROW, COL, NUM_LAYER>,
    keyboard_report_receiver: &Receiver<
        'b,
        CriticalSectionRawMutex,
        KeyboardReportMessage,
        REPORT_CHANNEL_SIZE,
    >,
) -> ! {
    loop {
        let usb_fut = usb_device.device.run();
        let keyboard_fut = keyboard.run();
        let matrix_fut = matrix.scan();
        let communication_fut = communication_task(
            keyboard_report_receiver,
            &mut usb_device.keyboard_hid_writer,
            &mut usb_device.other_hid_writer,
        );
        let led_fut = led_hid_task(&mut usb_device.keyboard_hid_reader, light_service);
        let via_fut = vial_task(&mut usb_device.via_hid, vial_service);

        pin_mut!(usb_fut);
        pin_mut!(keyboard_fut);
        pin_mut!(matrix_fut);
        pin_mut!(led_fut);
        pin_mut!(via_fut);
        pin_mut!(communication_fut);

        #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
        let storage_fut = storage.run();
        #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
        pin_mut!(storage_fut);

        match select4(
            select(usb_fut, keyboard_fut),
            #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
            select(storage_fut, via_fut),
            #[cfg(all(not(feature = "_nrf_ble"), feature = "_no_external_storage"))]
            #[cfg(feature = "_no_external_storage")]
            via_fut,
            led_fut,
            select(matrix_fut, communication_fut),
        )
        .await
        {
            Either4::First(_) => error!("Usb or keyboard task has died"),
            Either4::Second(_) => error!("Storage or vial task has died"),
            Either4::Third(_) => error!("Led task has died"),
            Either4::Fourth(_) => error!("Communication task has died"),
        }

        warn!("Detected failure, restarting keyboard sevice after 1 second");
        Timer::after_secs(1).await;
    }
}

pub(crate) fn reboot_keyboard() {
    warn!("Rebooting keyboard!");
    // For cortex-m:
    #[cfg(all(
        target_arch = "arm",
        target_os = "none",
        any(target_abi = "eabi", target_abi = "eabihf")
    ))]
    cortex_m::peripheral::SCB::sys_reset();

    #[cfg(feature = "_esp_ble")]
    esp_idf_svc::hal::reset::restart();
}
