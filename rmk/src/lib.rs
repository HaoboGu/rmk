#![doc = include_str!("../README.md")]
//! ## Feature flags
#![doc = document_features::document_features!()]
// Add docs.rs logo
#![doc(
    html_logo_url = "https://github.com/HaoboGu/rmk/blob/dad1f922f471127f5449262c4cb4a922e351bf43/docs/images/rmk_logo.svg?raw=true"
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

use core::cell::RefCell;
use core::future::Future;
use core::sync::atomic::Ordering;

use config::{RmkConfig, VialConfig};
use embassy_futures::select::{select4, Either4};
#[cfg(not(any(cortex_m)))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as RawMutex;
#[cfg(cortex_m)]
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex as RawMutex;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
use embedded_hal::digital::OutputPin;
use hid::{HidReaderTrait, HidWriterTrait, RunnableHidWriter};
use keymap::KeyMap;
use light::{LedIndicator, LightService};
use matrix::MatrixTrait;
use state::CONNECTION_STATE;
#[cfg(feature = "_ble")]
use trouble_host::prelude::*;
#[cfg(feature = "_ble")]
pub use trouble_host::prelude::{DefaultPacketPool, HostResources};
use usb::descriptor::ViaReport;
use via::VialService;
#[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]
use {
    crate::light::UsbLedReader,
    crate::usb::{add_usb_reader_writer, new_usb_builder, register_usb_writer, UsbKeyboardWriter},
};
#[cfg(feature = "storage")]
use {
    action::{EncoderAction, KeyAction},
    embassy_futures::select::select,
    embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash,
    storage::Storage,
};
pub use {embassy_futures, futures, heapless, rmk_macro as macros};
#[cfg(not(feature = "_ble"))]
use {
    usb::descriptor::{CompositeReport, KeyboardReport},
    via::UsbVialReaderWriter,
};

use crate::light::LightController;
use crate::state::ConnectionState;

pub mod action;
#[cfg(feature = "_ble")]
pub mod ble;
mod boot;
pub mod channel;
pub mod combo;
pub mod config;
pub mod controller;
pub mod debounce;
pub mod direct_pin;
pub mod event;
pub mod fork;
pub mod hid;
pub mod hid_state;
pub mod input_device;
pub mod keyboard;
pub mod keyboard_macros;
pub mod keycode;
pub mod keymap;
pub mod layout_macro;
pub mod light;
pub mod matrix;
#[cfg(feature = "split")]
pub mod split;
pub mod state;
#[cfg(feature = "storage")]
pub mod storage;
pub(crate) mod usb;
pub mod via;

pub async fn initialize_keymap<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    default_keymap: &mut [[[action::KeyAction; COL]; ROW]; NUM_LAYER],
    behavior_config: config::BehaviorConfig,
) -> RefCell<KeyMap<ROW, COL, NUM_LAYER>> {
    RefCell::new(KeyMap::new(default_keymap, None, behavior_config).await)
}

pub async fn initialize_encoder_keymap<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    default_keymap: &'a mut [[[action::KeyAction; COL]; ROW]; NUM_LAYER],
    default_encoder_map: &'a mut [[action::EncoderAction; NUM_ENCODER]; NUM_LAYER],
    behavior_config: config::BehaviorConfig,
) -> RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
    RefCell::new(KeyMap::new(default_keymap, Some(default_encoder_map), behavior_config).await)
}

#[cfg(feature = "storage")]
pub async fn initialize_encoder_keymap_and_storage<
    'a,
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    default_keymap: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    default_encoder_map: &'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER],
    flash: F,
    storage_config: &config::StorageConfig,
    behavior_config: config::BehaviorConfig,
) -> (
    RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    let mut storage = Storage::new(flash, default_keymap, &Some(default_encoder_map), storage_config).await;

    let keymap = RefCell::new(
        KeyMap::new_from_storage(
            default_keymap,
            Some(default_encoder_map),
            Some(&mut storage),
            behavior_config,
        )
        .await,
    );
    (keymap, storage)
}

#[cfg(feature = "storage")]
pub async fn initialize_keymap_and_storage<
    'a,
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    default_keymap: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    flash: F,
    storage_config: &config::StorageConfig,
    behavior_config: config::BehaviorConfig,
) -> (
    RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, 0>>,
    Storage<F, ROW, COL, NUM_LAYER, 0>,
) {
    let mut storage = Storage::new(flash, default_keymap, &None, storage_config).await;

    let keymap =
        RefCell::new(KeyMap::new_from_storage(default_keymap, None, Some(&mut storage), behavior_config).await);
    (keymap, storage)
}

#[allow(unreachable_code)]
pub async fn run_rmk<
    'a,
    'b,
    #[cfg(feature = "_ble")] C: Controller,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>, // TODO: remove the static lifetime
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(feature = "_ble")] stack: &'b Stack<'b, C, DefaultPacketPool>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    light_controller: &mut LightController<Out>,
    rmk_config: RmkConfig<'static>,
) -> ! {
    // Dispatch the keyboard runner
    #[cfg(feature = "_ble")]
    crate::ble::trouble::run_ble(
        keymap,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        #[cfg(feature = "_ble")]
        stack,
        #[cfg(feature = "storage")]
        storage,
        light_controller,
        rmk_config,
    )
    .await;

    // USB keyboard
    #[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]
    {
        let mut usb_builder: embassy_usb::Builder<'_, D> = new_usb_builder(usb_driver, rmk_config.usb_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let mut other_writer = register_usb_writer!(&mut usb_builder, CompositeReport, 9);
        let mut vial_reader_writer = add_usb_reader_writer!(&mut usb_builder, ViaReport, 32, 32);
        let (mut keyboard_reader, mut keyboard_writer) = keyboard_reader_writer.split();
        let mut usb_device = usb_builder.build();
        // Run all tasks, if one of them fails, wait 1 second and then restart
        loop {
            run_keyboard(
                keymap,
                #[cfg(feature = "storage")]
                storage,
                async { usb_device.run().await },
                light_controller,
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
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    communication_task: Fu,
    light_controller: &mut LightController<Out>,
    led_reader: R,
    vial_reader_writer: Rw,
    mut keyboard_writer: W,
    vial_config: VialConfig<'static>,
) {
    // The state will be changed to true after the keyboard starts running
    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
    let writer_fut = keyboard_writer.run_writer();
    let mut light_service = LightService::new(light_controller, led_reader);
    let mut vial_service = VialService::new(keymap, vial_config, vial_reader_writer);

    let led_fut = light_service.run();
    let via_fut = vial_service.run();

    #[cfg(feature = "storage")]
    let storage_fut = storage.run();
    match select4(
        communication_task,
        #[cfg(feature = "storage")]
        select(storage_fut, via_fut),
        #[cfg(not(feature = "storage"))]
        via_fut,
        led_fut,
        writer_fut,
    )
    .await
    {
        Either4::First(_) => error!("Communication task has ended"),
        Either4::Second(_) => error!("Storage or vial task has ended"),
        Either4::Third(_) => error!("Led task has ended"),
        Either4::Fourth(_) => error!("Keyboard writer task has ended"),
    }
    CONNECTION_STATE.store(ConnectionState::Disconnected.into(), Ordering::Release);
}
