use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{ItemFn, ItemMod};

use crate::{keyboard::Overwritten, ChipModel, ChipSeries};

// Default implementations of chip initialization
pub(crate) fn chip_init_default(chip: &ChipModel) -> TokenStream2 {
    match chip.series {
        ChipSeries::Stm32 => quote! {
                let config = ::embassy_stm32::Config::default();
                let mut p = ::embassy_stm32::init(config);
        },
        ChipSeries::Nrf52 => {
            quote! {
                    let config = ::embassy_nrf::config::Config::default();
                    config.gpiote_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
                    config.time_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
                    ::embassy_nrf::interrupt::USBD.set_priority(::embassy_nrf::interrupt::Priority::P2);
                    ::embassy_nrf::interrupt::POWER_CLOCK.set_priority(::embassy_nrf::interrupt::Priority::P2);
                    let p = ::embassy_nrf::init(nrf_config);
                    let clock: ::embassy_nrf::pac::CLOCK = unsafe { ::core::mem::transmute(()) };
                    info!("Enabling ext hfosc...");
                    clock.tasks_hfclkstart.write(|w| unsafe { w.bits(1) });
                    while clock.events_hfclkstarted.read().bits() != 1 {}
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
            let p = ::esp_idf_svc::peripherals::Peripherals::take().unwrap();
        },
        ChipSeries::Unsupported => quote! {},
    }
}

pub(crate) fn expand_chip_init(chip: &ChipModel, item_mod: &ItemMod) -> TokenStream2 {
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
                            return Some(override_chip_init(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(chip_init_default(chip))
    } else {
        chip_init_default(chip)
    }
}

fn override_chip_init(item_fn: &ItemFn) -> TokenStream2 {
    let initialization = item_fn.block.to_token_stream();
    quote! {
        let config = #initialization;
        let p = embassy_stm32::init(config);
    }
}
