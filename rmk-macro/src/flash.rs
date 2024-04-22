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
            let storage_config = ::rmk_config::StorageConfig::default();
        };
    }
    let num_sectors = storage_config.num_sectors;
    let start_addr = storage_config.start_addr;
    let mut flash_init = quote! {
        let storage_config = ::rmk_config::StorageConfig {
            num_sectors: #num_sectors,
            start_addr: #start_addr
        };
    };
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
                        let f = Nvmc::new(p.NVMC);
                    }
                } else {
                    // If BLE enables, RMK manages storage internally
                    quote! {}
                }
            }
            ChipSeries::Rp2040 => quote! {
                const FLASH_SIZE: usize = 2 * 1024 * 1024;
                let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, >::new(p.FLASH, p.DMA_CH0);
            },
            ChipSeries::Esp32 => quote! {},
            ChipSeries::Unsupported => quote! {},
        }
    );

    flash_init
}
