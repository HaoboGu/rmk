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
use config::RmkConfig;
#[cfg(feature = "controller")]
use controller::{PollingController, wpm::WpmController};
#[cfg(not(feature = "_ble"))]
use descriptor::{CompositeReport, KeyboardReport};
#[cfg(any(feature = "host", feature = "controller"))]
use embassy_futures::select::select;
use embassy_futures::select::{Either4, select4};
#[cfg(not(any(cortex_m)))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as RawMutex;
#[cfg(cortex_m)]
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex as RawMutex;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
use hid::{HidReaderTrait, RunnableHidWriter};
#[cfg(all(not(feature = "_ble"), feature = "host"))]
use host::UsbHostReaderWriter;
use keymap::KeyMap;
use matrix::MatrixTrait;
use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::led_indicator::LedIndicator;
use state::CONNECTION_STATE;
#[cfg(feature = "_ble")]
pub use trouble_host::prelude::*;
#[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]
use {
    crate::light::UsbLedReader,
    crate::usb::{UsbKeyboardWriter, add_usb_reader_writer, add_usb_writer, new_usb_builder},
};
#[cfg(feature = "host")]
use {config::VialConfig, descriptor::ViaReport, hid::HidWriterTrait, host::HostService};
pub use {embassy_futures, futures, heapless, rmk_macro as macros, rmk_types as types};
#[cfg(feature = "storage")]
use {embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash, storage::Storage};

use crate::config::PerKeyConfig;
use crate::keyboard::LOCK_LED_STATES;
use crate::state::ConnectionState;

#[cfg(feature = "bidirectional")]
pub mod bidirectional_matrix;
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
    key_info: &'a mut PerKeyConfig<ROW, COL>,
) -> RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>> {
    RefCell::new(KeyMap::new(default_keymap, None, behavior_config, key_info).await)
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
    key_info: &'a mut PerKeyConfig<ROW, COL>,
) -> RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
    RefCell::new(KeyMap::new(default_keymap, Some(default_encoder_map), behavior_config, key_info).await)
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
    key_info: &'a mut PerKeyConfig<ROW, COL>,
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
            &behavior_config,
        )
        .await;

        let keymap = RefCell::new(
            KeyMap::new_from_storage(
                default_keymap,
                Some(default_encoder_map),
                Some(&mut storage),
                behavior_config,
                key_info,
            )
            .await,
        );
        (keymap, storage)
    }

    #[cfg(not(feature = "host"))]
    {
        let storage = Storage::new(flash, storage_config, &behavior_config).await;
        let keymap =
            RefCell::new(KeyMap::new(default_keymap, Some(default_encoder_map), behavior_config, key_info).await);
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
    key_info: &'a mut PerKeyConfig<ROW, COL>,
) -> (
    RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, 0>>,
    Storage<F, ROW, COL, NUM_LAYER, 0>,
) {
    #[cfg(feature = "host")]
    {
        let mut storage = Storage::new(flash, default_keymap, &None, storage_config, &behavior_config).await;
        let keymap = RefCell::new(
            KeyMap::new_from_storage(default_keymap, None, Some(&mut storage), behavior_config, key_info).await,
        );
        (keymap, storage)
    }

    #[cfg(not(feature = "host"))]
    {
        let storage = Storage::new(flash, storage_config, &behavior_config).await;
        let keymap = RefCell::new(KeyMap::new(default_keymap, None, behavior_config, key_info).await);
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
        let mut usb_builder: embassy_usb::Builder<'_, D> = new_usb_builder(usb_driver, rmk_config.usb_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let mut other_writer = add_usb_writer!(&mut usb_builder, CompositeReport, 9);
        #[cfg(feature = "host")]
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
                    #[cfg(feature = "host")]
                    keymap,
                    #[cfg(feature = "storage")]
                    storage,
                    usb_task,
                    UsbLedReader::new(&mut keyboard_reader),
                    #[cfg(feature = "host")]
                    UsbHostReaderWriter::new(&mut vial_reader_writer),
                    UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                    #[cfg(feature = "host")]
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
    #[cfg(feature = "host")] Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
    R: HidReaderTrait<ReportType = LedIndicator>,
    W: RunnableHidWriter,
    Fu: Future<Output = ()>,
    #[cfg(feature = "storage")] F: AsyncNorFlash,
    #[cfg(any(feature = "storage", feature = "host"))] const ROW: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const COL: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_LAYER: usize,
    #[cfg(any(feature = "storage", feature = "host"))] const NUM_ENCODER: usize,
>(
    #[cfg(feature = "host")] keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    #[cfg(feature = "storage")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    communication_task: Fu,
    mut led_reader: R,
    #[cfg(feature = "host")] vial_reader_writer: Rw,
    mut keyboard_writer: W,
    #[cfg(feature = "host")] vial_config: VialConfig<'static>,
) {
    // The state will be changed to true after the keyboard starts running
    CONNECTION_STATE.store(ConnectionState::Connected.into(), Ordering::Release);
    let writer_fut = keyboard_writer.run_writer();
    #[cfg(feature = "host")]
    let mut vial_service = HostService::new(keymap, vial_config, vial_reader_writer);

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

    #[cfg(feature = "host")]
    let via_fut = vial_service.run();

    #[cfg(feature = "storage")]
    let storage_fut = storage.run();
    #[cfg(not(feature = "storage"))]
    let storage_fut = core::future::pending::<()>();

    #[cfg(feature = "controller")]
    let mut wpm_controller = WpmController::new();

    match select4(
        communication_task,
        #[cfg(feature = "host")]
        select(storage_fut, via_fut),
        #[cfg(not(feature = "host"))]
        storage_fut,
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
