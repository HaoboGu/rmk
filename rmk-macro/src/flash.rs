//! Initialize flash boilerplate of RMK, including USB or BLE
//!

use crate::config::StorageConfig;
use crate::{keyboard_config::KeyboardConfig, ChipSeries};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub(crate) fn expand_flash_init(keyboard_config: &KeyboardConfig) -> TokenStream2 {
    if !keyboard_config.storage.enabled {
        // This config actually does nothing if storage is disabled
        return quote! {
            let storage_config = ::rmk::config::StorageConfig::default();
            let flash = ::rmk::DummyFlash::new();
        };
    }
    let mut flash_init = get_storage_config(&keyboard_config.storage);
    flash_init.extend(
    match keyboard_config.chip.series {
            ChipSeries::Stm32 => {
                quote! {
                    let flash = ::rmk::storage::async_flash_wrapper(::embassy_stm32::flash::Flash::new_blocking(p.FLASH));
                }
            }
            ChipSeries::Nrf52 => {
                if !keyboard_config.communication.ble_enabled() {
                    // Usb only
                    quote! {
                        let flash = ::rmk::storage::async_flash_wrapper(::embassy_nrf::nvmc::Nvmc::new(p.NVMC));
                    }
                } else {
                    // If BLE enables, initialize both sd and flash
                    quote! {
                        let (sd, flash) = ::rmk::initialize_nrf_sd_and_flash("rmk", spawner, None);
                    }
                }
            }
            ChipSeries::Rp2040 => quote! {
                const FLASH_SIZE: usize = 2 * 1024 * 1024;
                let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);
            },
            ChipSeries::Esp32 => quote! {
                let flash = ::rmk::storage::async_flash_wrapper(unsafe {
                    ::esp_idf_svc::partition::EspPartition::new("rmk")
                        .expect("Create storage partition error")
                        .expect("Empty partition")
                });
            },
        }
    );

    flash_init
}

fn get_storage_config(storage_config: &StorageConfig) -> TokenStream2 {
    let num_sectors = storage_config.num_sectors.unwrap_or(2);
    let start_addr = storage_config.start_addr.unwrap_or(0);
    let clear_storage = storage_config.clear_storage.unwrap_or(false);
    quote! {
        let storage_config = ::rmk::config::StorageConfig {
            num_sectors: #num_sectors,
            start_addr: #start_addr,
            clear_storage: #clear_storage
        };
    }
}
