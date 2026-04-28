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
// Lints below fire inside `#[gatt_service]`/`#[gatt_server]` attribute-macro
// expansions from trouble-host; we can't annotate the generated code, so
// suppress them crate-wide rather than littering individual BLE structs.
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::needless_update)]
// Enable std for espidf and test
#![cfg_attr(not(test), no_std)]

// Mutual exclusivity guard
#[cfg(all(feature = "rmk_protocol", feature = "vial"))]
compile_error!("features `rmk_protocol` and `vial` are mutually exclusive");

// Re-export self as ::rmk for macro-generated code to work both inside and outside the crate
extern crate self as rmk;

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

// TODO: re-export to `constants`?
pub(crate) use rmk_types::constants::*;

// This mod MUST go first, so that the others see its macros.
pub(crate) mod fmt;

#[cfg(feature = "_ble")]
use bt_hci::{
    cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy},
    controller::{ControllerCmdAsync, ControllerCmdSync},
};
use config::RmkConfig;
pub use embassy_futures;
#[cfg(not(any(cortex_m)))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex as RawMutex;
#[cfg(cortex_m)]
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex as RawMutex;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
pub use futures;
pub use heapless;
use keymap::KeyMap;
pub use keymap::KeymapData;
pub use rmk_macro as macros;
pub use rmk_types as types;
#[cfg(all(feature = "storage", feature = "host"))]
use rmk_types::action::EncoderAction;
#[cfg(feature = "_ble")]
pub use trouble_host::prelude::*;
#[cfg(feature = "storage")]
use {embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash, storage::Storage};

use crate::config::PositionalConfig;

#[cfg(feature = "_ble")]
pub mod ble;
pub mod boot;
pub mod channel;
pub mod config;
pub mod core_traits;
pub mod debounce;
#[cfg(feature = "display")]
pub mod display;
pub mod driver;
pub mod event;
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
pub mod processor;
#[cfg(feature = "split")]
pub mod split;
pub mod state;
#[cfg(feature = "storage")]
pub mod storage;
#[cfg(not(feature = "_no_usb"))]
pub mod usb;

// Test-only helper that drives `embassy-time/mock-driver` from the
// `#[cfg(test)]` modules under `src/`. Mirrors the same helper at
// `tests/common/test_block_on.rs` (which is invisible to lib unit tests
// because integration tests are a separate compilation target).
#[cfg(test)]
pub(crate) mod test_support;

pub async fn initialize_keymap<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    data: &'a mut KeymapData<ROW, COL, NUM_LAYER, NUM_ENCODER>,
    behavior_config: &'a mut config::BehaviorConfig,
    positional_config: &'a PositionalConfig<ROW, COL>,
) -> KeyMap<'a> {
    KeyMap::new(data, behavior_config, positional_config).await
}

#[cfg(feature = "storage")]
pub async fn initialize_keymap_and_storage<
    'a,
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    data: &'a mut KeymapData<ROW, COL, NUM_LAYER, NUM_ENCODER>,
    flash: F,
    storage_config: &config::StorageConfig,
    behavior_config: &'a mut config::BehaviorConfig,
    positional_config: &'a PositionalConfig<ROW, COL>,
) -> (KeyMap<'a>, Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>) {
    #[cfg(feature = "host")]
    {
        let mut storage = {
            let encoder_opt: Option<&mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]> = if NUM_ENCODER > 0 {
                Some(&mut data.encoder_map)
            } else {
                None
            };
            Storage::new(flash, &data.keymap, &encoder_opt, storage_config, behavior_config).await
        };

        let keymap = KeyMap::new_from_storage(data, Some(&mut storage), behavior_config, positional_config).await;
        (keymap, storage)
    }

    #[cfg(not(feature = "host"))]
    {
        let storage = Storage::new(flash, storage_config, behavior_config).await;
        let keymap = KeyMap::new(data, behavior_config, positional_config).await;
        (keymap, storage)
    }
}

#[allow(unreachable_code)]
pub async fn run_rmk<
    #[cfg(feature = "_ble")] 'b,
    #[cfg(feature = "_ble")] C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
>(
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(feature = "_ble")] stack: &'b Stack<'b, C, DefaultPacketPool>,
    rmk_config: RmkConfig<'static>,
) -> ! {
    use core_traits::Runnable as _;

    use crate::processor::PollingProcessor;
    use crate::processor::builtin::wpm::WpmProcessor;

    #[cfg(feature = "_nrf_ble")]
    let rmk_config = {
        let mut config = rmk_config;
        crate::ble::apply_nrf_serial_number(&mut config);
        config
    };

    #[cfg(not(feature = "_no_usb"))]
    let device_config = rmk_config.device_config;

    let mut wpm = WpmProcessor::new();

    #[cfg(all(feature = "_ble", not(feature = "_no_usb")))]
    {
        let mut usb = crate::usb::UsbTransport::new(usb_driver, device_config);
        let mut ble = crate::ble::BleTransport::new(stack, rmk_config).await;
        embassy_futures::join::join3(usb.run(), ble.run(), wpm.polling_loop()).await;
    }

    #[cfg(all(feature = "_ble", feature = "_no_usb"))]
    {
        let mut ble = crate::ble::BleTransport::new(stack, rmk_config).await;
        embassy_futures::join::join(ble.run(), wpm.polling_loop()).await;
    }

    #[cfg(all(not(feature = "_ble"), not(feature = "_no_usb")))]
    {
        let mut usb = crate::usb::UsbTransport::new(usb_driver, device_config);
        embassy_futures::join::join(usb.run(), wpm.polling_loop()).await;
    }

    unreachable!("Should never reach here, wrong feature gate combination?");
}
