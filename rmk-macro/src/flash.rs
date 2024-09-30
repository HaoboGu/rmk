//! Initialize flash boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::StorageConfig;

use crate::{keyboard::CommunicationType, ChipModel, ChipSeries};

pub(crate) fn expand_flash_init(
    chip: &ChipModel,
    communication_type: CommunicationType,
    storage_config: StorageConfig,
) -> TokenStream2 {
    if !storage_config.enabled {
        // This config actually does nothing if storage is disabled
        return quote! {
            let storage_config = ::rmk::config::StorageConfig::default();
        };
    }
    let mut flash_init = get_storage_config(chip, storage_config);
    flash_init.extend(
    match chip.series {
            ChipSeries::Stm32 => {
                quote! {
                    let f = ::embassy_stm32::flash::Flash::new_blocking(p.FLASH);
                }
            }
            ChipSeries::Nrf52 => {
                if communication_type == CommunicationType::Usb {
                    // Usb only
                    quote! {
                        let f = ::embassy_nrf::nvmc::Nvmc::new(p.NVMC);
                    }
                } else {
                    // If BLE enables, RMK manages storage internally
                    quote! {}
                }
            }
            ChipSeries::Rp2040 => quote! {
                const FLASH_SIZE: usize = 2 * 1024 * 1024;
                let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);
            },
            ChipSeries::Esp32 => quote! {},
        }
    );

    flash_init
}

fn get_storage_config(chip: &ChipModel, storage_config: StorageConfig) -> TokenStream2 {
    let (num_sectors, start_addr) = match chip.series {
        ChipSeries::Nrf52 => {
            // Special default config for nRF52
            // It's common to use [Adafruit_nRF52_Bootloader](https://github.com/adafruit/Adafruit_nRF52_Bootloader) for nRF52 chips, we don't want our default storage config breaks the bootloader
            let start_addr = if storage_config.start_addr == 0x0000_0000 {
                0x0006_0000
            } else {
                storage_config.start_addr
            };
            let num_sectors = if storage_config.num_sectors == 2 {
                6
            } else {
                storage_config.num_sectors as usize
            };
            (num_sectors as u8, start_addr)
        },
        _ => (storage_config.num_sectors, storage_config.start_addr),
    };
    quote! {
        let storage_config = ::rmk::config::StorageConfig {
            num_sectors: #num_sectors,
            start_addr: #start_addr
        };
    }
}