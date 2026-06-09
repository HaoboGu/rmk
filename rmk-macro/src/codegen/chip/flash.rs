//! Initialize flash boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::resolved::Hardware;
use rmk_config::resolved::hardware::{ChipSeries, DfuConfig};

use super::gpio::convert_gpio_str_to_output_pin;

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
    let _start_addr = storage.start_addr;
    let clear_storage = storage.clear_storage;
    let clear_layout = storage.clear_layout;

    // With dfu-rp, the flash is already a partition that starts at the
    // storage offset, so the relative offset must be 0.
    #[cfg(feature = "dfu-rp")]
    let storage_start_addr = 0usize;
    #[cfg(not(feature = "dfu-rp"))]
    let storage_start_addr = _start_addr;

    let mut flash_init = quote! {
        let storage_config = ::rmk::config::StorageConfig {
            num_sectors: #num_sectors,
            start_addr: #storage_start_addr,
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
            #[cfg(not(feature = "dfu-rp"))]
            {
                quote! {
                    const FLASH_SIZE: usize = 2 * 1024 * 1024;
                    let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, FLASH_SIZE>::new(
                        p.FLASH, p.DMA_CH1, Irqs,
                    );
                }
            }
            #[cfg(feature = "dfu-rp")]
            {
                let storage = hardware.storage.as_ref();
                let storage_start = storage.map(|s| s.start_addr).unwrap_or(0) as u32;
                let storage_num_sectors = storage.map(|s| s.num_sectors).unwrap_or(0) as u32;
                let erase_size = 4096u32;
                let storage_end = if storage_start == 0 {
                    2 * 1024 * 1024 - erase_size * storage_num_sectors
                } else {
                    storage_start + storage_num_sectors * erase_size
                };
                let dfu = hardware.dfu.as_ref().expect(
                    "[dfu] section is required in keyboard.toml (or chip default) when dfu-rp is enabled"
                );
                let state_offset = dfu.state_offset;
                let state_size = dfu.state_size;
                let dfu_offset = dfu.dfu_offset;
                let dfu_size = dfu.dfu_size;
                let dfu_led = dfu.led.as_ref().map(|c| {
                    convert_gpio_str_to_output_pin(&hardware.chip, c.pin.clone(), false)
                });
                let dfu_led_init = match dfu_led {
                    Some(pin) => quote! {
                        ::rmk::dfu::set_led(Some(#pin));
                    },
                    None => quote! {},
                };
                let dfu_unlock_keys = expand_dfu_unlock_keys(dfu);
                quote! {
                    #dfu_unlock_keys
                    let flash = ::rmk::storage::async_flash_wrapper(
                        ::rmk::dfu::init_flash(
                            p.FLASH,
                            #storage_start,
                            #storage_end,
                            #state_offset,
                            #state_size,
                            #dfu_offset,
                            #dfu_size,
                        )
                    );
                    #dfu_led_init
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

/// Generate the `DFU_UNLOCK_KEYS` constant from the resolved DFU config.
fn expand_dfu_unlock_keys(dfu: &DfuConfig) -> TokenStream2 {
    if dfu.unlock_keys.is_empty() {
        return quote! {};
    }
    let keys_expr = dfu
        .unlock_keys
        .iter()
        .map(|key| {
            let row = key[0];
            let col = key[1];
            quote! { (#row, #col) }
        })
        .collect::<Vec<_>>();
    quote! {
        const DFU_UNLOCK_KEYS: &[(u8, u8)] = &[#(#keys_expr), *];
    }
}
