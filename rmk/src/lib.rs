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
#![allow(async_fn_in_trait)]
// Enable std for espidf and test
#![cfg_attr(not(test), no_std)]

// Include generated constants
include!(concat!(env!("OUT_DIR"), "/constants.rs"));

// This mod MUST go first, so that the others see its macros.
pub(crate) mod fmt;

#[cfg(feature = "_esp_ble")]
use crate::ble::esp::run_esp_ble_keyboard;
#[cfg(feature = "_nrf_ble")]
pub use crate::ble::nrf::initialize_nrf_sd_and_flash;
use crate::light::LightController;
use config::{RmkConfig, VialConfig};
use core::{
    cell::RefCell,
    future::Future,
    sync::atomic::{AtomicBool, AtomicU8},
};
use embassy_futures::select::{select, select4, Either4};
#[cfg(not(any(cortex_m)))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as RawMutex;
#[cfg(cortex_m)]
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex as RawMutex;
use embassy_time::Timer;
use embassy_usb::{driver::Driver, UsbDevice};
use embedded_hal;
use embedded_hal::digital::OutputPin;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
pub use futures;
use hid::{HidReaderTrait, HidWriterTrait, RunnableHidWriter};
use keymap::KeyMap;
use light::{LedIndicator, LightService};
use matrix::MatrixTrait;
#[cfg(feature = "_nrf_ble")]
use nrf_softdevice::Softdevice;
pub use rmk_macro as macros;
use storage::Storage;
pub use storage::dummy_flash::DummyFlash;
use usb::descriptor::ViaReport;
use via::VialService;
#[cfg(all(not(feature = "_nrf_ble"), not(feature = "_no_usb")))]
use {
    crate::light::UsbLedReader,
    crate::usb::descriptor::{CompositeReport, KeyboardReport},
    crate::usb::{UsbKeyboardWriter, new_usb_builder},
    crate::via::UsbVialReaderWriter,
};

pub use heapless;
#[cfg(not(feature = "_no_usb"))]
use usb::{add_usb_reader_writer, register_usb_writer};

pub mod action;
#[cfg(feature = "_ble")]
pub mod ble;
mod boot;
pub mod channel;
pub mod combo;
pub mod config;
pub mod debounce;
pub mod direct_pin;
pub mod event;
pub mod hid;
pub mod input_device;
pub mod keyboard;
mod keyboard_macro;
pub mod keycode;
pub mod keymap;
pub mod layout_macro;
pub mod light;
pub mod matrix;
#[cfg(feature = "split")]
pub mod split;
pub mod storage;
pub(crate) mod usb;
pub mod via;

/// Current connection type:
/// - 0: USB
/// - 1: BLE
/// - Other: reserved
pub(crate) static CONNECTION_TYPE: AtomicU8 = AtomicU8::new(0);
/// Whether the connection is ready.
/// After the connection is ready, the matrix starts scanning
pub(crate) static CONNECTION_STATE: AtomicBool = AtomicBool::new(false);

pub async fn initialize_keymap_and_storage<
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    default_keymap: &mut [[[action::KeyAction; COL]; ROW]; NUM_LAYER],
    flash: F,
    storage_config: config::StorageConfig,
    behavior_config: config::BehaviorConfig,
) -> (
    RefCell<KeyMap<ROW, COL, NUM_LAYER>>,
    Storage<F, ROW, COL, NUM_LAYER>,
) {
    let mut storage = Storage::new(flash, default_keymap, storage_config).await;

    let keymap = RefCell::new(
        KeyMap::new_from_storage(default_keymap, Some(&mut storage), behavior_config).await,
    );
    (keymap, storage)
}

#[allow(unreachable_code)]
pub async fn run_rmk<
    'a,
    F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    mut storage: Storage<F, ROW, COL, NUM_LAYER>,
    mut light_controller: LightController<Out>,
    rmk_config: RmkConfig<'static>,
    #[cfg(feature = "_nrf_ble")] sd: &mut Softdevice,
) -> ! {
    // Dispatch the keyboard runner
    #[cfg(feature = "_nrf_ble")]
    crate::ble::nrf::run_nrf_ble_keyboard(
        keymap,
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
    let writer_fut = keyboard_writer.run_writer();
    let mut light_service = LightService::new(light_controller, led_reader);
    let mut vial_service = VialService::new(keymap, vial_config, vial_reader_writer);

    let led_fut = light_service.run();
    let via_fut = vial_service.run();

    #[cfg(any(feature = "_ble", not(feature = "_no_external_storage")))]
    let storage_fut = storage.run();

    match select4(
        communication_task,
        #[cfg(any(feature = "_ble", not(feature = "_no_external_storage")))]
        select(storage_fut, via_fut),
        #[cfg(all(not(feature = "_ble"), feature = "_no_external_storage"))]
        via_fut,
        led_fut,
        writer_fut,
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
