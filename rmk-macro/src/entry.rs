use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemFn, ItemMod};

use crate::{
    keyboard::Overwritten,
    keyboard_config::{CommunicationConfig, KeyboardConfig},
    ChipSeries,
};

pub(crate) fn expand_rmk_entry(
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
                        if let Ok(Overwritten::Entry) =
                            Overwritten::from_meta(&item_fn.attrs[0].meta)
                        {
                            return Some(override_rmk_entry(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(rmk_entry_default(keyboard_config))
    } else {
        rmk_entry_default(keyboard_config)
    }
}

fn override_rmk_entry(item_fn: &ItemFn) -> TokenStream2 {
    let content = &item_fn.block.stmts;
    quote! {
        #(#content)*
    }
}

pub(crate) fn rmk_entry_default(keyboard_config: &KeyboardConfig) -> TokenStream2 {
    match keyboard_config.chip.series {
        ChipSeries::Stm32 => {
            quote! {
                ::rmk::run_rmk(
                    input_pins,
                    output_pins,
                    driver,
                    f,
                    &mut get_default_keymap(),
                    keyboard_config,
                    spawner,
                )
                .await;
            }
        }
        ChipSeries::Nrf52 => match keyboard_config.communication {
            CommunicationConfig::Usb(_) => {
                quote! {
                    ::rmk::run_rmk(
                        input_pins,
                        output_pins,
                        driver,
                        f,
                        &mut get_default_keymap(),
                        keyboard_config,
                        spawner
                    )
                    .await;
                }
            }
            CommunicationConfig::Both(_, _) => quote! {
                ::rmk::run_rmk(
                    input_pins,
                    output_pins,
                    driver,
                    &mut get_default_keymap(),
                    keyboard_config,
                    spawner,
                )
                .await;
            },
            CommunicationConfig::Ble(_) => quote! {
                ::rmk::run_rmk(
                    input_pins,
                    output_pins,
                    &mut get_default_keymap(),
                    keyboard_config,
                    spawner,
                )
                .await;
            },
            CommunicationConfig::None => quote! {},
        },
        ChipSeries::Rp2040 => quote! {
            ::rmk::run_rmk_with_async_flash(
                input_pins,
                output_pins,
                driver,
                flash,
                &mut get_default_keymap(),
                keyboard_config,
                spawner,
            )
            .await;
        },
        ChipSeries::Esp32 => quote! {
            ::esp_idf_svc::hal::task::block_on(::rmk::run_rmk(
                input_pins,
                output_pins,
                &mut get_default_keymap(),
                keyboard_config,
            ));
        },
    }
}
