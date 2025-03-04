use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemFn, ItemMod};

use crate::{
    keyboard::Overwritten,
    keyboard_config::{BoardConfig, CommunicationConfig, KeyboardConfig},
    ChipSeries,
};

pub(crate) fn expand_rmk_entry(
    keyboard_config: &KeyboardConfig,
    item_mod: &ItemMod,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(entry)]`, override the entry
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
            .unwrap_or(rmk_entry_select(keyboard_config))
    } else {
        rmk_entry_select(keyboard_config)
    }
}

fn override_rmk_entry(item_fn: &ItemFn) -> TokenStream2 {
    let content = &item_fn.block.stmts;
    quote! {
        #(#content)*
    }
}

pub(crate) fn rmk_entry_select(keyboard_config: &KeyboardConfig) -> TokenStream2 {
    match &keyboard_config.board {
        BoardConfig::Split(_) => {
            quote! { compile_error!("You should use #[rmk_central] or #[rmk_peripheral] macro instead.");}
        }
        _ => rmk_entry_default(keyboard_config),
    }
}

pub(crate) fn rmk_entry_default(keyboard_config: &KeyboardConfig) -> TokenStream2 {
    match keyboard_config.chip.series {
        ChipSeries::Nrf52 => match keyboard_config.communication {
            CommunicationConfig::Usb(_) => {
                quote! {
                    ::rmk::futures::future::join(
                        ::rmk::bind_device_and_processor_and_run!((matrix) => keyboard),
                        ::rmk::run_rmk(&keymap, driver, storage, light_controller, rmk_config),
                    ).await;
                }
            }
            CommunicationConfig::Both(_, _) => quote! {
                ::rmk::futures::future::join(
                    ::rmk::bind_device_and_processor_and_run!((matrix) => keyboard),
                    ::rmk::run_rmk(&keymap, driver, storage, light_controller, rmk_config, sd),
                ).await;
            },
            CommunicationConfig::Ble(_) => quote! {
                ::rmk::futures::future::join(
                    ::rmk::bind_device_and_processor_and_run!((matrix) => keyboard),
                    ::rmk::run_rmk(&keymap, storage, light_controller, rmk_config, sd),
                ).await;
            },
            CommunicationConfig::None => quote! {},
        },
        ChipSeries::Esp32 => quote! {
            ::esp_idf_svc::hal::task::block_on(
                ::rmk::futures::future::join(
                    ::rmk::bind_device_and_processor_and_run!((matrix) => keyboard),
                    ::rmk::run_rmk(&keymap, storage, light_controller, rmk_config),
                )
            );
        },
        _ => quote! {
            ::rmk::futures::future::join(
                ::rmk::bind_device_and_processor_and_run!((matrix) => keyboard),
                ::rmk::run_rmk(&keymap, driver, storage, light_controller, rmk_config),
            ).await;
        },
    }
}
