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
use crate::config::RmkConfig;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::input_device::InputProcessor;
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
pub use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::*};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embassy_usb::UsbDevice;
pub use embedded_hal;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
#[cfg(not(feature = "_no_external_storage"))]
use embedded_storage::nor_flash::NorFlash;
pub use flash::EmptyFlashWrapper;
use futures::pin_mut;
use keyboard::{Keyboard, KEYBOARD_REPORT_CHANNEL};
pub use keyboard::{EVENT_CHANNEL, EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE};
use keymap::KeyMap;
use matrix::{Matrix, MatrixTrait};
use reporter::{HidReporter as _, UsbKeyboardReporter};
pub use rmk_macro as macros;
use usb::descriptor::{CompositeReport, KeyboardReport, ViaReport};
use usb::{new_usb_builder, register_usb_reader_writer, register_usb_writer};
use via::process::VialService;
#[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
use {embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash, storage::Storage};

pub mod action;
#[cfg(feature = "_ble")]
pub mod ble;
pub mod config;
pub mod debounce;
pub mod direct_pin;
pub mod event;
mod flash;
mod hid;
pub mod input_device;
pub mod keyboard;
mod keyboard_macro;
pub mod keycode;
mod keymap;
mod layout_macro;
mod light;
pub mod matrix;
pub mod reporter;
#[cfg(feature = "split")]
pub mod split;
mod storage;
#[macro_use]
mod usb;
mod via;

/// Keyboard state, true for started, false for stopped
pub(crate) static KEYBOARD_STATE: AtomicBool = AtomicBool::new(false);
/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
/// Whethe the connection is ready.
/// After the connection is ready, the matrix starts scanning
pub(crate) static CONNECTION_STATE: AtomicBool = AtomicBool::new(false);

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

pub async fn initialize_usb_keyboard_and_run<
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

    // Create keyboard services and devices
    let (mut keyboard, mut vial_service, mut light_service) = (
        Keyboard::new(&keymap, keyboard_config.behavior_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    let mut usb_builder = new_usb_builder(usb_driver, keyboard_config.usb_config);
    let keyboard_reader_writer =
        register_usb_reader_writer::<_, KeyboardReport, 1, 8>(&mut usb_builder);
    let other_writer = register_usb_writer::<_, CompositeReport, 9>(&mut usb_builder);
    let via_reader_writer = register_usb_reader_writer::<_, ViaReport, 32, 32>(&mut usb_builder);
    let (keyboard_reader, keyboard_writer) = keyboard_reader_writer.split();
    let mut usb_reporter = UsbKeyboardReporter {
        keyboard_writer,
        other_writer,
    };

    let mut usb_device = usb_builder.build();

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
        &mut usb_reporter,
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
    usb_device: &mut UsbDevice<'a, D>,
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
    usb_reporter: &mut UsbKeyboardReporter<'a, D>,
) -> ! {
    loop {
        CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
        let usb_fut = usb_device.run();
        let keyboard_fut = keyboard.run();
        let matrix_fut = matrix.run();
        let reporter_fut = usb_reporter.run();
        // FIXME: add led and vial back
        // let led_fut = led_hid_task(&mut usb_device.keyboard_hid_reader, light_service);
        // let via_fut = vial_task(&mut usb_device.via_hid, vial_service);

        pin_mut!(usb_fut);
        pin_mut!(keyboard_fut);
        pin_mut!(matrix_fut);
        // pin_mut!(led_fut);
        // pin_mut!(via_fut);
        pin_mut!(reporter_fut);

        #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
        let storage_fut = storage.run();
        #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
        pin_mut!(storage_fut);

        match select4(
            select(usb_fut, keyboard_fut),
            #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
            storage_fut,
            // #[cfg(all(not(feature = "_nrf_ble"), feature = "_no_external_storage"))]
            // via_fut,
            // led_fut,
            matrix_fut,
            reporter_fut,
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
