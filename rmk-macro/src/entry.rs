use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
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
        BoardConfig::Split(split_config) => {
            let matrix_task = quote! {
                ::rmk::bind_device_and_processor_and_run!((matrix) => keyboard)
            };
            match keyboard_config.chip.series {
                ChipSeries::Stm32 | ChipSeries::Rp2040 => {
                    let rmk_task = quote! {
                        ::rmk::run_rmk(&keymap, driver, storage, light_controller, rmk_config),
                    };
                    let mut tasks = vec![matrix_task, rmk_task];
                    let central_serials = split_config
                        .central
                        .serial
                        .clone()
                        .expect("No serial defined for central");
                    split_config
                        .peripheral
                        .iter()
                        .enumerate()
                        .for_each(|(idx, p)| {
                            let row = p.rows;
                            let col = p.cols;
                            let row_offset = p.row_offset;
                            let col_offset = p.col_offset;
                            let uart_instance = format_ident!("{}", central_serials.get(idx).expect("No or not enough serial defined for peripheral in central").instance.to_lowercase());
                            tasks.push(quote! {
                                ::rmk::split::central::run_peripheral_manager::<#row, #col, #row_offset, #col_offset, _>(
                                    #idx,
                                    #uart_instance,
                                )
                            });
                        });
                    join_all_tasks(tasks)
                }
                ChipSeries::Nrf52 => {
                    let rmk_task = quote! {
                        ::rmk::run_rmk(&keymap, driver, storage, light_controller, rmk_config, sd),
                    };
                    let mut tasks = vec![matrix_task, rmk_task];
                    split_config.peripheral.iter().enumerate().for_each(|(idx, p)| {
                        let row = p.rows ;
                        let col = p.cols ;
                        let row_offset = p.row_offset ;
                        let col_offset = p.col_offset ;
                        let peripheral_ble_addr = p.ble_addr.expect("No ble_addr defined for peripheral");
                        tasks.push(quote! {
                            ::rmk::split::central::run_peripheral_manager::<#row, #col, #row_offset, #col_offset>(
                                #idx,
                                [#(#peripheral_ble_addr), *],
                            )
                        });
                    });
                    join_all_tasks(tasks)
                }
                ChipSeries::Esp32 => panic!("Split for esp32 isn't implemented yet"),
            }
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

pub(crate) fn join_all_tasks(tasks: Vec<TokenStream2>) -> TokenStream2 {
    let mut current_joined = quote! {};
    tasks.iter().enumerate().for_each(|(id, task)| {
        if id == 0 {
            current_joined = quote! {#task};
        } else {
            current_joined = quote! {
                ::embassy_futures::join::join(#current_joined, #task)
            };
        }
    });

    quote! {
        #current_joined.await;
    }
}
