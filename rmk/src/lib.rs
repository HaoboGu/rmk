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

// This mod MUST go first, so that the others see its macros.
pub(crate) mod fmt;

use crate::config::KeyboardConfig;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::input_device::InputProcessor;
use crate::light::LightController;
use action::KeyAction;
use config::{RmkConfig, VialConfig};
use core::{
    cell::RefCell,
    future::Future,
    sync::atomic::{AtomicBool, AtomicU8},
};
use debounce::DebouncerTrait;
#[cfg(not(feature = "_esp_ble"))]
use embassy_executor::Spawner;
use embassy_futures::select::{select, select4, Either4};
#[cfg(not(any(cortex_m)))]
pub use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex as RawMutex, channel::*};
// #[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(any(cortex_m))]
pub use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex as RawMutex, channel::*};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embassy_usb::UsbDevice;
pub use embedded_hal;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
#[cfg(not(feature = "_no_external_storage"))]
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
pub use flash::EmptyFlashWrapper;
use futures::pin_mut;
pub use heapless;
use hid::{HidReaderTrait, HidWriterTrait, RunnableHidWriter};
use keyboard::{communication_task, Keyboard, KeyboardReportMessage, KEYBOARD_REPORT_CHANNEL};
pub use keyboard::{EVENT_CHANNEL, EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE};
use keymap::KeyMap;
use light::{LedIndicator, LightService};
use matrix::{Matrix, MatrixTrait};
pub use rmk_macro as macros;
pub use storage::dummy_flash::DummyFlash;
use storage::Storage;
use usb::descriptor::ViaReport;
use via::VialService;
#[cfg(feature = "_esp_ble")]
use {crate::ble::esp::run_esp_ble_keyboard, esp_idf_svc::partition::EspPartition};
#[cfg(feature = "_nrf_ble")]
use {
    crate::ble::nrf::{initialize_nrf_sd_and_flash, run_nrf_ble_keyboard},
    nrf_softdevice::Softdevice,
};
#[cfg(all(not(feature = "_nrf_ble"), not(feature = "_no_usb")))]
use {
    crate::light::UsbLedReader,
    crate::usb::descriptor::{CompositeReport, KeyboardReport},
    crate::usb::{new_usb_builder, UsbKeyboardWriter},
    crate::via::UsbVialReaderWriter,
};

pub mod action;
#[cfg(feature = "_ble")]
pub mod ble;
pub mod channel;
pub mod combo;
pub mod config;
pub mod debounce;
pub mod direct_pin;
pub mod event;
mod hid;
pub mod input_device;
pub mod keyboard;
mod keyboard_macro;
pub mod keycode;
mod keymap;
mod layout_macro;
mod light;
pub mod matrix;
#[cfg(feature = "split")]
pub mod split;
mod storage;
#[macro_use]
mod usb;
mod via;

/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
/// Whether the connection is ready.
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
/// * `keyboard_config` - other configurations of the keyboard, check [KeyboardConfig] struct for details
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
    keyboard_config: KeyboardConfig<'static, Out>,
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    // Wrap `embedded-storage` to `embedded-storage-async`
    #[cfg(not(feature = "_no_external_storage"))]
    let flash = embassy_embedded_hal::adapter::BlockingAsync::new(flash);
    run_rmk_with_async_flash(
        input_pins,
        output_pins,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        #[cfg(not(feature = "_no_external_storage"))]
        flash,
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
/// * `keyboard_config` - other configurations of the keyboard, check [KeyboardConfig] struct for details
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
#[allow(unused_variables)]
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
    keyboard_config: KeyboardConfig<'static, Out>,
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    let rmk_config = keyboard_config.rmk_config;
    #[cfg(feature = "_nrf_ble")]
    let (sd, flash) =
        initialize_nrf_sd_and_flash(rmk_config.usb_config.product_name, spawner, None);

    #[cfg(feature = "_esp_ble")]
    let flash = {
        let f = unsafe {
            EspPartition::new("rmk")
                .expect("Create storage partition error")
                .expect("Empty partition")
        };
        let async_flash = embassy_embedded_hal::adapter::BlockingAsync::new(f);
        async_flash
    };

    let mut storage = Storage::new(flash, default_keymap, rmk_config.storage_config).await;
    let keymap = RefCell::new(
        KeyMap::new_from_storage(
            default_keymap,
            Some(&mut storage),
            keyboard_config.behavior_config,
        )
        .await,
    );
    let keyboard = Keyboard::new(&keymap, rmk_config.behavior_config);
    let light_controller = LightController::new(keyboard_config.controller_config.light_config);

    // Create the debouncer, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let debouncer = RapidDebouncer::<ROW, COL>::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer = RapidDebouncer::<COL, ROW>::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<COL, ROW> = DefaultDebouncer::<COL, ROW>::new();

    // Keyboard matrix, use COL2ROW by default
    #[cfg(feature = "col2row")]
    let matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let matrix = Matrix::<_, _, _, COL, ROW>::new(input_pins, output_pins, debouncer);

    run_rmk_internal(
        matrix,   // matrix input device
        keyboard, // key processor
        &keymap,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        storage,
        light_controller,
        rmk_config,
        #[cfg(feature = "_nrf_ble")]
        sd,
    )
    .await
}

#[allow(unreachable_code)]
pub(crate) async fn run_rmk_internal<
    'a,
    M: MatrixTrait,
    F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    mut matrix: M,
    mut keyboard: Keyboard<'a, ROW, COL, NUM_LAYER>,
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    mut storage: Storage<F, ROW, COL, NUM_LAYER>,
    mut light_controller: LightController<Out>,
    rmk_config: RmkConfig<'static>,
    #[cfg(feature = "_nrf_ble")] sd: &mut Softdevice,
) -> ! {
    // Dispatch the keyboard runner
    #[cfg(feature = "_nrf_ble")]
    run_nrf_ble_keyboard(
        keymap,
        &mut keyboard,
        &mut matrix,
        &mut storage,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        &mut light_controller,
        rmk_config,
        sd,
    )
    .await;

    #[cfg(feature = "_esp_ble")]
    run_esp_ble_keyboard(
        keymap,
        &mut keyboard,
        &mut matrix,
        &mut storage,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        &mut light_controller,
        rmk_config,
    )
    .await;

    // USB keyboard
    #[cfg(all(
        not(feature = "_nrf_ble"),
        not(feature = "_no_usb"),
        not(feature = "_esp_ble")
    ))]
    {
        let mut usb_builder: embassy_usb::Builder<'_, D> =
            new_usb_builder(usb_driver, rmk_config.usb_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let mut other_writer = register_usb_writer!(&mut usb_builder, CompositeReport, 9);
        let mut vial_reader_writer = add_usb_reader_writer!(&mut usb_builder, ViaReport, 32, 32);
        let (mut keyboard_reader, mut keyboard_writer) = keyboard_reader_writer.split();
        let mut usb_device = usb_builder.build();
        // Run all tasks, if one of them fails, wait 1 second and then restart
        loop {
            run_keyboard(
                keymap,
                &mut keyboard,
                &mut matrix,
                &mut storage,
                run_usb_device(&mut usb_device),
                &mut light_controller,
                UsbLedReader::new(&mut keyboard_reader),
                UsbVialReaderWriter::new(&mut vial_reader_writer),
                UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                rmk_config.vial_config,
            )
            .await;
        }
    }

    unreachable!("Should never reach here, wrong feature gate combination?");
}

// Run keyboard task for once
pub(crate) async fn run_keyboard<
    'a,
    M: MatrixTrait,
    Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
    R: HidReaderTrait<ReportType = LedIndicator>,
    W: RunnableHidWriter,
    Fu: Future<Output = ()>,
    F: AsyncNorFlash,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    keyboard: &mut Keyboard<'a, ROW, COL, NUM_LAYER>,
    matrix: &mut M,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER>,
    communication_task: Fu,
    light_controller: &mut LightController<Out>,
    led_reader: R,
    vial_reader_writer: Rw,
    mut keyboard_writer: W,
    vial_config: VialConfig<'static>,
) {
    // The state will be changed to true after the keyboard starts running
    CONNECTION_STATE.store(false, core::sync::atomic::Ordering::Release);
    let keyboard_fut = keyboard.run();
    let matrix_fut = matrix.run();
    let writer_fut = keyboard_writer.run_writer();
    let mut light_service = LightService::new(light_controller, led_reader);
    let mut vial_service = VialService::new(&keymap, vial_config, vial_reader_writer);

    let led_fut = light_service.run();
    let via_fut = vial_service.run();

    #[cfg(any(feature = "_ble", not(feature = "_no_external_storage")))]
    let storage_fut = storage.run();

    match select4(
        select(communication_task, keyboard_fut),
        #[cfg(any(feature = "_ble", not(feature = "_no_external_storage")))]
        select(storage_fut, via_fut),
        #[cfg(all(not(feature = "_ble"), feature = "_no_external_storage"))]
        via_fut,
        led_fut,
        select(matrix_fut, writer_fut),
    )
    .await
    {
        Either4::First(_) => error!("Communication or keyboard task has died"),
        Either4::Second(_) => error!("Storage or vial task has died"),
        Either4::Third(_) => error!("Led task has died"),
        Either4::Fourth(_) => error!("Matrix or writer task has died"),
    }

    warn!("Detected failure, restarting keyboard sevice after 1 second");
    Timer::after_secs(1).await;
}

pub(crate) async fn run_usb_device<'d, D: Driver<'d>>(usb_device: &mut UsbDevice<'d, D>) {
    usb_device.run().await;
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

/// Runnable trait defines `run` function for running the task
pub trait Runnable {
    /// Run function
    fn run(&mut self) -> impl Future<Output = ()>;
}
