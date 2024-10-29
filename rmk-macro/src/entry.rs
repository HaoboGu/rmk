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
        BoardConfig::Normal(_) => rmk_entry_default(keyboard_config),
        BoardConfig::DirectPin(_) => rmk_entry_direct_pin(keyboard_config),
        BoardConfig::Split(_) => {
            quote! { compile_error!("You should use #[rmk_central] or #[rmk_peripheral] macro instead.");}
        }
    }
}

pub(crate) fn rmk_entry_direct_pin(keyboard_config: &KeyboardConfig) -> TokenStream2 {
    match keyboard_config.chip.series {
        ChipSeries::Stm32 => {
            quote! {
                // `generic_arg_infer` is a nightly feature. Const arguments cannot yet be inferred with `_` in stable now.
                ::rmk::direct_pin::run_rmk_direct_pin::<_, ::embassy_stm32::gpio::Output, _, _, ROW, COL, SIZE, NUM_LAYER>(
                    direct_pins,
                    driver,
                    f,
                    &mut get_default_keymap(),
                    keyboard_config,
                    low_active,
                    spawner,
                )
                .await;
            }
        }
        ChipSeries::Nrf52 => match keyboard_config.communication {
            CommunicationConfig::Usb(_) => {
                quote! {
                    ::rmk::direct_pin::run_rmk_direct_pin::<_, ::embassy_nrf::gpio::Output, _, _, ROW, COL, SIZE, NUM_LAYER>(
                        direct_pins,
                        driver,
                        f,
                        &mut get_default_keymap(),
                        keyboard_config,
                        low_active,
                        spawner
                    )
                    .await;
                }
            }
            CommunicationConfig::Both(_, _) => quote! {
                ::rmk::direct_pin::run_rmk_direct_pin::<_, ::embassy_nrf::gpio::Output, _, ROW, COL, SIZE, NUM_LAYER>(
                    direct_pins,
                    driver,
                    &mut get_default_keymap(),
                    keyboard_config,
                    low_active,
                    spawner,
                )
                .await;
            },
            CommunicationConfig::Ble(_) => quote! {
                ::rmk::direct_pin::run_rmk_direct_pin::<_, ::embassy_nrf::gpio::Output, ROW, COL, SIZE, NUM_LAYER>(
                    direct_pins,
                    &mut get_default_keymap(),
                    keyboard_config,
                    low_active,
                    spawner,
                )
                .await;
            },
            CommunicationConfig::None => quote! {},
        },
        ChipSeries::Rp2040 => quote! {
            ::rmk::direct_pin::run_rmk_direct_pin_with_async_flash::<_, ::embassy_rp::gpio::Output, _, _, ROW, COL, SIZE, NUM_LAYER>(
                direct_pins,
                driver,
                flash,
                &mut get_default_keymap(),
                keyboard_config,
                low_active,
                spawner,
            )
            .await;
        },
        ChipSeries::Esp32 => quote! {
            ::esp_idf_svc::hal::task::block_on(::rmk::direct_pin::run_rmk_direct_pin::<_, ::esp_idf_svc::hal::gpio::Output, ROW, COL, SIZE, NUM_LAYER>(
                direct_pins,
                &mut get_default_keymap(),
                keyboard_config,
                low_active,
            ));
        },
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
