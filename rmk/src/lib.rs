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

// Re-export self as ::rmk for macro-generated code to work both inside and outside the crate
extern crate self as rmk;

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
use config::RmkConfig;
#[cfg(feature = "controller")]
use controller::{PollingController, wpm::WpmController};
#[cfg(not(feature = "_ble"))]
use descriptor::{CompositeReport, KeyboardReport};
#[cfg(not(any(cortex_m)))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as RawMutex;
#[cfg(cortex_m)]
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex as RawMutex;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
use futures::FutureExt;
use hid::{HidReaderTrait, RunnableHidWriter};
use keymap::KeyMap;
use matrix::MatrixTrait;
use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::led_indicator::LedIndicator;
use state::CONNECTION_STATE;
#[cfg(feature = "_ble")]
pub use trouble_host::prelude::*;
#[cfg(feature = "host")]
use {crate::descriptor::ViaReport, crate::hid::HidWriterTrait, crate::host::run_host_communicate_task};
#[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]
use {
    crate::light::UsbLedReader,
    crate::usb::{UsbKeyboardWriter, add_usb_reader_writer, add_usb_writer, new_usb_builder},
};
pub use {embassy_futures, futures, heapless, rmk_macro as macros, rmk_types as types};
#[cfg(feature = "storage")]
use {embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash, storage::Storage};

use crate::config::PositionalConfig;
#[cfg(feature = "vial")]
use crate::config::VialConfig;
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
pub mod helper_macro;
pub mod hid;
#[cfg(feature = "host")]
pub mod host;
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

pub async fn initialize_keymap<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    default_keymap: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    behavior_config: &'a mut config::BehaviorConfig,
    positional_config: &'a mut PositionalConfig<ROW, COL>,
) -> RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>> {
    RefCell::new(KeyMap::new(default_keymap, None, behavior_config, positional_config).await)
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
    positional_config: &'a mut PositionalConfig<ROW, COL>,
) -> RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
    RefCell::new(
        KeyMap::new(
            default_keymap,
            Some(default_encoder_map),
            behavior_config,
            positional_config,
        )
        .await,
    )
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
    positional_config: &'a mut PositionalConfig<ROW, COL>,
) -> (
    RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    #[cfg(feature = "host")]
    {
        let mut storage = Storage::new(
            flash,
            default_keymap,
            &Some(default_encoder_map),
            storage_config,
            behavior_config,
        )
        .await;

        let keymap = RefCell::new(
            KeyMap::new_from_storage(
                default_keymap,
                Some(default_encoder_map),
                Some(&mut storage),
                behavior_config,
                positional_config,
            )
            .await,
        );
        (keymap, storage)
    }

    #[cfg(not(feature = "host"))]
    {
        let storage = Storage::new(flash, storage_config, &behavior_config).await;
        let keymap = RefCell::new(
            KeyMap::new(
                default_keymap,
                Some(default_encoder_map),
                behavior_config,
                positional_config,
            )
            .await,
        );
        (keymap, storage)
    }
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
    positional_config: &'a mut PositionalConfig<ROW, COL>,
) -> (
    RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, 0>>,
    Storage<F, ROW, COL, NUM_LAYER, 0>,
) {
    #[cfg(feature = "host")]
    {
        let mut storage = Storage::new(flash, default_keymap, &None, storage_config, behavior_config).await;
        let keymap = RefCell::new(
            KeyMap::new_from_storage(
                default_keymap,
                None,
                Some(&mut storage),
                behavior_config,
                positional_config,
            )
            .await,
        );
        (keymap, storage)
    }

    #[cfg(not(feature = "host"))]
    {
        let storage = Storage::new(flash, storage_config, &behavior_config).await;
        let keymap = RefCell::new(KeyMap::new(default_keymap, None, behavior_config, positional_config).await);
        (keymap, storage)
    }
}

#[allow(unreachable_code)]
pub async fn run_rmk<
    'a,
    #[cfg(feature = "_ble")] 'b,
    #[cfg(feature = "_ble")] C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(any(feature = "storage", feature = "host"))] const ROW: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const COL: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_LAYER: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_ENCODER: usize,
>(
    #[cfg(feature = "host")] keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(feature = "_ble")] stack: &'b Stack<'b, C, DefaultPacketPool>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    rmk_config: RmkConfig<'static>,
) -> ! {
    // Dispatch the keyboard runner
    #[cfg(feature = "_ble")]
    crate::ble::run_ble(
        #[cfg(feature = "host")]
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
        let mut usb_builder: embassy_usb::Builder<'_, D> = new_usb_builder(usb_driver, rmk_config.device_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let mut other_writer = add_usb_writer!(&mut usb_builder, CompositeReport, 9);
        #[cfg(feature = "host")]
        let mut host_reader_writer = add_usb_reader_writer!(&mut usb_builder, ViaReport, 32, 32);

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
                    #[cfg(feature = "storage")]
                    storage,
                    #[cfg(feature = "host")]
                    keymap,
                    #[cfg(feature = "host")]
                    crate::host::UsbHostReaderWriter::new(&mut host_reader_writer),
                    #[cfg(feature = "vial")]
                    rmk_config.vial_config,
                    usb_task,
                    UsbLedReader::new(&mut keyboard_reader),
                    UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                )
                .await;
            }
        })
        .await;
    }

    unreachable!("Should never reach here, wrong feature gate combination?");
}

// Run keyboard task for once
//
// Due to https://github.com/rust-lang/rust/issues/62958, storage/host struct is used now.
// The corresponding future(commented) will be used after the issue is fixed.
pub(crate) async fn run_keyboard<
    'a,
    R: HidReaderTrait<ReportType = LedIndicator>,
    W: RunnableHidWriter,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(feature = "host")] Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
    #[cfg(any(feature = "storage", feature = "host"))] const ROW: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const COL: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_LAYER: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_ENCODER: usize,
>(
    // #[cfg(feature = "storage")] storage_task: impl Future<Output = ()>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    // #[cfg(feature = "host")] host_task: impl Future<Output = ()>,
    #[cfg(feature = "host")] keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(feature = "host")] reader_writer: Rw,
    #[cfg(feature = "vial")] vial_config: VialConfig<'static>,
    communication_fut: impl Future<Output = ()>,
    mut led_reader: R,
    mut keyboard_writer: W,
) {
    // The state will be changed to true after the keyboard starts running
    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
    let writer_fut = keyboard_writer.run_writer();
    let led_fut = async {
        loop {
            match led_reader.read_report().await {
                Ok(led_indicator) => {
                    info!("Got led indicator");
                    LOCK_LED_STATES.store(led_indicator.into_bits(), core::sync::atomic::Ordering::Relaxed);
                    #[cfg(feature = "controller")]
                    crate::event::publish_controller_event(crate::event::LedIndicatorEvent {
                        indicator: led_indicator,
                    });
                }
                Err(e) => {
                    error!("Read HID LED indicator error: {:?}", e);
                    embassy_time::Timer::after_millis(1000).await
                }
            }
        }
    };

    #[cfg(feature = "host")]
    let host_fut = run_host_communicate_task(
        keymap,
        reader_writer,
        #[cfg(feature = "vial")]
        vial_config,
    );
    #[cfg(feature = "storage")]
    let storage_fut = storage.run();

    #[cfg(feature = "controller")]
    let mut wpm_controller = WpmController::new();

    #[cfg(feature = "storage")]
    let storage_task = core::pin::pin!(storage_fut.fuse());
    #[cfg(feature = "host")]
    let host_task = core::pin::pin!(host_fut.fuse());
    let mut communication_task = core::pin::pin!(communication_fut.fuse());
    let mut led_task = core::pin::pin!(led_fut.fuse());
    let mut writer_task = core::pin::pin!(writer_fut.fuse());

    futures::select_biased! {
        _ = communication_task => error!("Communication task has ended"),
        _ = with_feature!("storage", storage_task) => error!("Storage task has ended"),
        _ = with_feature!("controller", wpm_controller.polling_loop()) => error!("WPM Controller task ended"),
        _ = led_task => error!("Led task has ended"),
        _ = with_feature!("host", host_task) => error!("Host task ended"),
        _ = writer_task => error!("Writer task has ended"),
    };

    CONNECTION_STATE.store(ConnectionState::Disconnected.into(), Ordering::Release);
}
