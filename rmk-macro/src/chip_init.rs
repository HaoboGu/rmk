use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{ItemFn, ItemMod};

use crate::{keyboard::Overwritten, keyboard_config::KeyboardConfig, ChipModel, ChipSeries};

// Default implementations of chip initialization
pub(crate) fn chip_init_default(chip: &ChipModel) -> TokenStream2 {
    match chip.series {
        ChipSeries::Stm32 => quote! {
                let config = ::embassy_stm32::Config::default();
                let mut p = ::embassy_stm32::init(config);
        },
        ChipSeries::Nrf52 => {
            let usb_related_config = if chip.has_usb() {
                quote! {
                    ::embassy_nrf::interrupt::USBD.set_priority(::embassy_nrf::interrupt::Priority::P2);
                }
            } else {
                quote! {}
            };
            quote! {
                use embassy_nrf::{ interrupt::InterruptExt, config::Reg0Voltage};
                let mut config = ::embassy_nrf::config::Config::default();
                
                // https://github.com/embassy-rs/embassy/issues/3134
                config.dcdc.reg0 = true;
                config.dcdc.reg0_voltage = Some(Reg0Voltage::_3v3);
                // config.hfclk_source = ::embassy_nrf::config::HfclkSource::ExternalXtal;
                // config.lfclk_source = ::embassy_nrf::config::LfclkSource::ExternalXtal;
                config.gpiote_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
                config.time_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
                #usb_related_config
                ::embassy_nrf::interrupt::POWER_CLOCK.set_priority(::embassy_nrf::interrupt::Priority::P2);
                let p = ::embassy_nrf::init(config);
                // Disable external HF clock by default, reduce power consumption
                // let clock: ::embassy_nrf::pac::CLOCK = unsafe { ::core::mem::transmute(()) };
                // info!("Enabling ext hfosc...");
                // clock.tasks_hfclkstart.write(|w| unsafe { w.bits(1) });
                // while clock.events_hfclkstarted.read().bits() != 1 {}
            }
        }
        ChipSeries::Rp2040 => {
            quote! {
                let config = ::embassy_rp::config::Config::default();
                let p = ::embassy_rp::init(config);
            }
        }
        ChipSeries::Esp32 => quote! {
            ::esp_idf_svc::sys::link_patches();
            ::esp_idf_svc::log::EspLogger::initialize_default();
            let p = ::esp_idf_svc::hal::peripherals::Peripherals::take().unwrap();
        },
    }
}

pub(crate) fn expand_chip_init(
    keyboard_config: &KeyboardConfig,
    item_mod: &ItemMod,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Ok(Overwritten::ChipConfig) =
                            Overwritten::from_meta(&item_fn.attrs[0].meta)
                        {
                            return Some(override_chip_init(&keyboard_config.chip, item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(chip_init_default(&keyboard_config.chip))
    } else {
        chip_init_default(&keyboard_config.chip)
    }
}

fn override_chip_init(chip: &ChipModel, item_fn: &ItemFn) -> TokenStream2 {
    let initialization = item_fn.block.to_token_stream();
    let mut initialization_tokens = quote! {
        let config = #initialization;
    };
    match chip.series {
        ChipSeries::Stm32 => initialization_tokens.extend(quote! {
            let mut p = ::embassy_stm32::init(config);
        }),
        ChipSeries::Nrf52 => initialization_tokens.extend(quote! {
            let mut p = ::embassy_nrf::init(config);
        }),
        ChipSeries::Rp2040 => initialization_tokens.extend(quote! {
            let mut p = ::embassy_rp::init(config);
        }),
        ChipSeries::Esp32 => initialization_tokens.extend(quote! {
            let p = ::esp_idf_svc::hal::peripherals::Peripherals::take().unwrap();
        }),
    }

    initialization_tokens
}
