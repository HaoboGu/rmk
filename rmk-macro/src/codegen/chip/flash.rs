//! Initialize flash boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::resolved::Hardware;
use rmk_config::resolved::hardware::ChipSeries;

pub(crate) fn expand_flash_init(hardware: &Hardware) -> TokenStream2 {
    if hardware.storage.is_none() {
        // This config actually does nothing if storage is disabled
        return quote! {
            // let storage_config = ::rmk::config::StorageConfig::default();
            // let flash = ::rmk::DummyFlash::new();
        };
    }
    let storage = hardware.storage.as_ref().unwrap();
    let num_sectors = storage.num_sectors;
    let start_addr = storage.start_addr;
    let clear_storage = storage.clear_storage;
    let clear_layout = storage.clear_layout;
    let mut flash_init = quote! {
        let storage_config = ::rmk::config::StorageConfig {
            num_sectors: #num_sectors,
            start_addr: #start_addr,
            clear_storage: #clear_storage,
            clear_layout: #clear_layout
        };
    };
    flash_init.extend(
    match hardware.chip.series {
            ChipSeries::Stm32 => {
                quote! {
                    let flash = ::rmk::storage::async_flash_wrapper(::embassy_stm32::flash::Flash::new_blocking(p.FLASH));
                }
            }
            ChipSeries::Nrf52 => {
                quote! {
                    let flash = ::nrf_mpsl::Flash::take(mpsl, p.NVMC);
                }
            }
            ChipSeries::Rp2040 => {
                if hardware.communication.ble_enabled() {
                    quote! {
                        const FLASH_SIZE: usize = 2 * 1024 * 1024;
                        let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, FLASH_SIZE>::new(
                            p.FLASH,
                            p.DMA_CH1,
                            Irqs,
                        );
                    }
                } else {
                    quote! {
                        const FLASH_SIZE: usize = 2 * 1024 * 1024;
                        let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, FLASH_SIZE>::new(
                            p.FLASH,
                            p.DMA_CH1,
                            Irqs,
                        );
                    }
                }
            }
            ChipSeries::Esp32 => quote! {
                let flash = ::rmk::storage::async_flash_wrapper(::esp_storage::FlashStorage::new(p.FLASH));
            },
        }
    );

    flash_init
}
