//! Initialize flash boilerplate of RMK, including USB or BLE
//!

use crate::codegen::feature::is_feature_enabled;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::resolved::Hardware;
use rmk_config::resolved::hardware::{ChipSeries, ExternalFlashDriver, SpiConfig};

#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
use rmk_config::resolved::hardware::DfuConfig;

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

    // With dfu, the flash is already a partition that starts at the
    // storage offset, so the relative offset must be 0.
    #[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
    let storage_start_addr = 0usize;
    #[cfg(not(any(feature = "dfu_rp", feature = "dfu_nrf")))]
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
                #[cfg(feature = "dfu_nrf")]
                let flash_code = {
                    let dfu = hardware.dfu.as_ref().expect(
                        "[dfu] section is required in keyboard.toml (or chip default) when dfu_nrf is enabled"
                    );
                    let dfu_unlock_keys = expand_dfu_unlock_keys(dfu);
                    let external_dfu = expand_external_flash_init(hardware);
                    let flash_let = if external_dfu.is_some() {
                        quote! {
                            let flash = ::rmk::storage::async_flash_wrapper(
                                ::rmk::dfu::init_flash_from_linkerscript_with_external_dfu(
                                    p.NVMC,
                                    dfu_mutex,
                                )
                            );
                        }
                    } else {
                        quote! {
                            let flash = ::rmk::storage::async_flash_wrapper(
                                ::rmk::dfu::init_flash_from_linkerscript(p.NVMC)
                            );
                        }
                    };
                    quote! {
                        #dfu_unlock_keys
                        #external_dfu
                        #flash_let
                    }
                };
                #[cfg(not(feature = "dfu_nrf"))]
                let flash_code = quote! {
                    let flash = ::nrf_mpsl::Flash::take(mpsl, p.NVMC);
                };
                flash_code
            }
        ChipSeries::Rp2040 => {
            #[cfg(not(feature = "dfu_rp"))]
            {
                quote! {
                    const FLASH_SIZE: usize = 2 * 1024 * 1024;
                    let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, FLASH_SIZE>::new(
                        p.FLASH, p.DMA_CH1, Irqs,
                    );
                }
            }
            #[cfg(feature = "dfu_rp")]
            {
                let dfu = hardware.dfu.as_ref().expect(
                    "[dfu] section is required in keyboard.toml (or chip default) when dfu_rp is enabled"
                );
                let dfu_unlock_keys = expand_dfu_unlock_keys(dfu);
                let external_dfu = expand_external_flash_init(hardware);
                let flash_let = if external_dfu.is_some() {
                    quote! {
                        let flash = ::rmk::storage::async_flash_wrapper(
                            ::rmk::dfu::init_flash_from_linkerscript_with_external_dfu(
                                p.FLASH,
                                dfu_mutex,
                            )
                        );
                    }
                } else {
                    quote! {
                        let flash = ::rmk::storage::async_flash_wrapper(
                            ::rmk::dfu::init_flash_from_linkerscript(p.FLASH)
                        );
                    }
                };
                quote! {
                    #dfu_unlock_keys
                    #external_dfu
                    #flash_let
                }
            }
            }
            ChipSeries::Esp32 => {
                // ESP32 and ESP32-S3 are dual-core. Flash writes must auto-park it to avoid
                // `FlashStorageError::OtherCoreRunning`.
                let chip_name = hardware.chip.chip.to_lowercase();
                if chip_name == "esp32s3"{
                    quote! {
                        let flash = ::rmk::storage::async_flash_wrapper(
                            ::esp_storage::FlashStorage::new(p.FLASH).multicore_auto_park()
                        );
                    }
                } else {
                    quote! {
                        let flash = ::rmk::storage::async_flash_wrapper(::esp_storage::FlashStorage::new(p.FLASH));
                    }
                }
            },
        }
    );

    flash_init
}

/// Generate the `DFU_UNLOCK_KEYS` constant from the resolved DFU config.
#[cfg(any(feature = "dfu_rp", feature = "dfu_nrf"))]
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

/// Generate external SPI flash initialization for DFU.
///
/// Creates the external flash and wraps it in a `'static` mutex. The tokens
/// must run inside `expand_flash_init`, directly before
/// `init_flash_from_linkerscript_with_external_dfu`.
///
/// Returns `None` if `dfu_ext` is not enabled or no external flash is
/// configured.
fn expand_external_flash_init(hardware: &Hardware) -> Option<TokenStream2> {
    let rmk_features = crate::codegen::feature::get_rmk_features();
    if !is_feature_enabled(&rmk_features, "dfu_ext") {
        return None;
    }
    let ext_flash = hardware.dfu.as_ref()?.external_flash.as_ref()?;
    let spi_init = expand_spi_init(&hardware.chip.series, &ext_flash.spi);
    let (spi_ty, cs_ty) = expand_flash_driver_type(&hardware.chip.series, &ext_flash.spi);
    let flash_ty = quote! {
        ::rmk::driver::w25q::W25qNorFlash<#spi_ty, #cs_ty>
    };
    let flash_init = match &ext_flash.driver {
        ExternalFlashDriver::W25q => {
            let size = ext_flash.flash_size;
            quote! {
                let ext_flash = ::rmk::driver::w25q::W25qNorFlash::new(dfu_spi, dfu_cs, #size);
            }
        }
        ExternalFlashDriver::Custom => {
            panic!(
                "[dfu.external_flash] driver = \"custom\" is not supported by #[rmk_keyboard]; \
                 use a `use_rust` setup calling `rmk::dfu::init_flash_from_linkerscript_with_external_dfu` manually"
            )
        }
    };
    Some(quote! {
        #spi_init
        #flash_init
        static EXT_DFU_MUTEX: ::static_cell::StaticCell<
            ::embassy_sync::blocking_mutex::Mutex<
                ::embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
                core::cell::RefCell<#flash_ty>,
            >
        > = ::static_cell::StaticCell::new();
        let dfu_mutex = EXT_DFU_MUTEX.init(
            ::embassy_sync::blocking_mutex::Mutex::new(core::cell::RefCell::new(ext_flash))
        );
    })
}

/// The concrete SPI and CS driver types, per chip series.
///
/// `instance` is the SPI peripheral name from the config (e.g. `"SPI1"`),
/// formatted into an identifier.
fn expand_flash_driver_type(
    chip_series: &ChipSeries,
    spi: &SpiConfig,
) -> (TokenStream2, TokenStream2) {
    let instance = format_ident!("{}", spi.instance);
    match chip_series {
        ChipSeries::Rp2040 => (
            quote! { ::embassy_rp::spi::Spi<'static, ::embassy_rp::peripherals::#instance, ::embassy_rp::spi::Blocking> },
            quote! { ::embassy_rp::gpio::Output<'static> },
        ),
        ChipSeries::Nrf52 => (
            quote! { ::embassy_nrf::spim::Spim<'static> },
            quote! { ::embassy_nrf::gpio::Output<'static> },
        ),
        _ => panic!("External flash DFU is only supported on RP2040 and nRF52"),
    }
}

fn expand_spi_init(chip_series: &ChipSeries, spi: &SpiConfig) -> TokenStream2 {
    let instance = format_ident!("{}", spi.instance);
    match chip_series {
        ChipSeries::Rp2040 => {
            let sck = format_ident!("{}", spi.sck);
            let mosi = format_ident!("{}", spi.mosi);
            let miso = format_ident!("{}", spi.miso);
            let cs = format_ident!("{}", spi.cs.as_ref().unwrap());
            quote! {
                let dfu_spi = ::embassy_rp::spi::Spi::new_blocking(
                    p.#instance,
                    p.#sck,
                    p.#mosi,
                    p.#miso,
                    ::embassy_rp::spi::Config::default(),
                );
                let dfu_cs = ::embassy_rp::gpio::Output::new(
                    p.#cs,
                    ::embassy_rp::gpio::Level::High,
                );
            }
        }
        ChipSeries::Nrf52 => {
            let sck = format_ident!("{}", spi.sck);
            let mosi = format_ident!("{}", spi.mosi);
            let miso = format_ident!("{}", spi.miso);
            let cs = format_ident!("{}", spi.cs.as_ref().unwrap());
            quote! {
                let mut dfu_spi_cfg = ::embassy_nrf::spim::Config::default();
                dfu_spi_cfg.frequency = ::embassy_nrf::spim::Frequency::M8;
                let dfu_spi = ::embassy_nrf::spim::Spim::new(
                    p.#instance, Irqs,
                    p.#sck, p.#miso, p.#mosi, dfu_spi_cfg,
                );
                let dfu_cs = ::embassy_nrf::gpio::Output::new(
                    p.#cs,
                    ::embassy_nrf::gpio::Level::High,
                    ::embassy_nrf::gpio::OutputDrive::Standard,
                );
            }
        }
        _ => panic!("External flash DFU is only supported on RP2040 and nRF52"),
    }
}
