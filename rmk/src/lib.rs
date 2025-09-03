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

#[cfg(feature = "_ble")]
use bt_hci::{
    cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy},
    controller::{ControllerCmdAsync, ControllerCmdSync},
};
use config::{RmkConfig, VialConfig};
#[cfg(feature = "controller")]
use controller::{PollingController, wpm::WpmController};
use descriptor::ViaReport;
use embassy_futures::select::{Either4, select4};
#[cfg(not(any(cortex_m)))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as RawMutex;
#[cfg(cortex_m)]
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex as RawMutex;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
use hid::{HidReaderTrait, HidWriterTrait, RunnableHidWriter};
use keymap::KeyMap;
use matrix::MatrixTrait;
use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::led_indicator::LedIndicator;
use state::CONNECTION_STATE;
#[cfg(feature = "_ble")]
use trouble_host::prelude::*;
#[cfg(feature = "_ble")]
pub use trouble_host::prelude::{DefaultPacketPool, HostResources};
use via::VialService;
#[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]
use {
    crate::light::UsbLedReader,
    crate::usb::{UsbKeyboardWriter, add_usb_reader_writer, add_usb_writer, new_usb_builder},
};
#[cfg(not(feature = "_ble"))]
use {
    descriptor::{CompositeReport, KeyboardReport},
    via::UsbVialReaderWriter,
};
pub use {embassy_futures, futures, heapless, rmk_macro as macros, rmk_types as types};
#[cfg(feature = "storage")]
use {embassy_futures::select::select, embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash, storage::Storage};

use crate::keyboard::LOCK_LED_STATES;
use crate::state::ConnectionState;

#[cfg(feature = "_ble")]
pub mod ble;
mod boot;
pub mod channel;
pub mod combo;
pub mod config;
#[cfg(feature = "controller")]
pub mod controller;
pub mod debounce;
pub mod descriptor;
pub mod direct_pin;
pub mod driver;
pub mod event;
pub mod fork;
pub mod hid;
pub mod input_device;
pub mod keyboard;
pub mod keyboard_macros;
pub mod keymap;
pub mod layout_macro;
pub mod light;
pub mod matrix;
pub mod morse;
#[cfg(feature = "split")]
pub mod split;
pub mod state;
#[cfg(feature = "storage")]
pub mod storage;
#[cfg(not(feature = "_no_usb"))]
pub mod usb;
pub mod via;

pub async fn initialize_keymap<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    default_keymap: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    behavior_config: &'a mut config::BehaviorConfig,
) -> RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>> {
    RefCell::new(KeyMap::new(default_keymap, None, behavior_config).await)
}

pub async fn initialize_encoder_keymap<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    default_keymap: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    default_encoder_map: &'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER],
    behavior_config: &'a mut config::BehaviorConfig,
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
    behavior_config: &'a mut config::BehaviorConfig,
) -> (
    RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    let mut storage = Storage::new(
        flash,
        default_keymap,
        &Some(default_encoder_map),
        storage_config,
        &behavior_config,
    )
    .await;

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
    behavior_config: &'a mut config::BehaviorConfig,
) -> (
    RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, 0>>,
    Storage<F, ROW, COL, NUM_LAYER, 0>,
) {
    let mut storage = Storage::new(flash, default_keymap, &None, storage_config, &behavior_config).await;

    let keymap =
        RefCell::new(KeyMap::new_from_storage(default_keymap, None, Some(&mut storage), behavior_config).await);
    (keymap, storage)
}

#[allow(unreachable_code)]
pub async fn run_rmk<
    'a,
    #[cfg(feature = "_ble")] 'b,
    #[cfg(feature = "_ble")] C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(feature = "_ble")] stack: &'b Stack<'b, C, DefaultPacketPool>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
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
        rmk_config,
    )
    .await;

    // USB keyboard
    #[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]
    {
        let mut usb_builder: embassy_usb::Builder<'_, D> = new_usb_builder(usb_driver, rmk_config.usb_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let mut other_writer = add_usb_writer!(&mut usb_builder, CompositeReport, 9);
        let mut vial_reader_writer = add_usb_reader_writer!(&mut usb_builder, ViaReport, 32, 32);
        let (mut keyboard_reader, mut keyboard_writer) = keyboard_reader_writer.split();

        #[cfg(feature = "usb_log")]
        let logger_fut = {
            let usb_logger = crate::usb::add_usb_logger!(&mut usb_builder);
            embassy_usb_logger::with_class!(1024, log::LevelFilter::Debug, usb_logger)
        };
        #[cfg(not(feature = "usb_log"))]
        let logger_fut = async {};
        let mut usb_device = usb_builder.build();

        // Run all tasks, if one of them fails, wait 1 second and then restart
        embassy_futures::join::join(logger_fut, async {
            loop {
                let usb_task = async {
                    loop {
                        use embassy_futures::select::{Either, select};

                        use crate::usb::USB_REMOTE_WAKEUP;

                        // Run
                        usb_device.run_until_suspend().await;
                        // Suspended, wait resume or remote wakeup
                        match select(usb_device.wait_resume(), USB_REMOTE_WAKEUP.wait()).await {
                            Either::First(_) => continue,
                            Either::Second(_) => {
                                info!("USB wakeup remote");
                            }
                        }
                    }
                };

                run_keyboard(
                    keymap,
                    #[cfg(feature = "storage")]
                    storage,
                    usb_task,
                    UsbLedReader::new(&mut keyboard_reader),
                    UsbVialReaderWriter::new(&mut vial_reader_writer),
                    UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                    rmk_config.vial_config,
                )
                .await;
            }
        })
        .await;
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
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    communication_task: Fu,
    mut led_reader: R,
    vial_reader_writer: Rw,
    mut keyboard_writer: W,
    vial_config: VialConfig<'static>,
) {
    // The state will be changed to true after the keyboard starts running
    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
    let writer_fut = keyboard_writer.run_writer();
    let mut vial_service = VialService::new(keymap, vial_config, vial_reader_writer);

    let led_fut = async {
        #[cfg(feature = "controller")]
        let mut controller_pub = unwrap!(crate::channel::CONTROLLER_CHANNEL.publisher());
        loop {
            match led_reader.read_report().await {
                Ok(led_indicator) => {
                    info!("Got led indicator");
                    LOCK_LED_STATES.store(led_indicator.into_bits(), core::sync::atomic::Ordering::Relaxed);
                    #[cfg(feature = "controller")]
                    {
                        info!("Publishing led indicator");
                        // Publish the event
                        crate::channel::send_controller_event(
                            &mut controller_pub,
                            crate::event::ControllerEvent::KeyboardIndicator(led_indicator),
                        );
                    }
                }
                Err(e) => {
                    error!("Read HID LED indicator error: {:?}", e);
                    embassy_time::Timer::after_millis(1000).await
                }
            }
        }
    };

    let via_fut = vial_service.run();

    #[cfg(feature = "storage")]
    let storage_fut = storage.run();

    #[cfg(feature = "controller")]
    let mut wpm_controller = WpmController::new();

    match select4(
        communication_task,
        #[cfg(feature = "storage")]
        select(storage_fut, via_fut),
        #[cfg(not(feature = "storage"))]
        via_fut,
        #[cfg(feature = "controller")]
        select(wpm_controller.polling_loop(), led_fut),
        #[cfg(not(feature = "controller"))]
        led_fut,
        writer_fut,
    )
    .await
    {
        Either4::First(_) => error!("Communication task has ended"),
        Either4::Second(_) => error!("Storage or vial task has ended"),
        Either4::Third(_) => error!("Controller or led task has ended"),
        Either4::Fourth(_) => error!("Keyboard writer task has ended"),
    }
    CONNECTION_STATE.store(ConnectionState::Disconnected.into(), Ordering::Release);
}
