use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{ItemFn, ItemMod};

use crate::keyboard::Overwritten;
use crate::keyboard_config::{BoardConfig, CommunicationConfig, KeyboardConfig};
use crate::ChipSeries;

pub(crate) fn expand_rmk_entry(
    keyboard_config: &KeyboardConfig,
    item_mod: &ItemMod,
    devices: Vec<TokenStream2>,
    processors: Vec<TokenStream2>,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(entry)]`, override the entry
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Ok(Overwritten::Entry) = Overwritten::from_meta(&item_fn.attrs[0].meta) {
                            return Some(override_rmk_entry(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(rmk_entry_select(keyboard_config, devices, processors))
    } else {
        rmk_entry_select(keyboard_config, devices, processors)
    }
}

fn override_rmk_entry(item_fn: &ItemFn) -> TokenStream2 {
    let content = &item_fn.block.stmts;
    quote! {
        #(#content)*
    }
}

pub(crate) fn rmk_entry_select(
    keyboard_config: &KeyboardConfig,
    devices: Vec<TokenStream2>,
    processors: Vec<TokenStream2>,
) -> TokenStream2 {
    let devices_task = {
        let mut devs = devices.clone();
        devs.push(quote! {matrix});
        quote! {
            ::rmk::run_devices! (
                (#(#devs),*) => ::rmk::channel::EVENT_CHANNEL,
            )
        }
    };
    let processors_task = if processors.is_empty() {
        quote! {}
    } else {
        quote! {
            ::rmk::run_processor_chain! (
                ::rmk::channel::EVENT_CHANNEL=> [#(#processors),*],
            )
        }
    };
    let entry = match &keyboard_config.board {
        BoardConfig::Split(split_config) => {
            let keyboard_task = quote! {
                keyboard.run(),
            };
            match keyboard_config.chip.series {
                ChipSeries::Stm32 | ChipSeries::Rp2040 => {
                    let rmk_task = quote! {
                        ::rmk::run_rmk(&keymap, driver, &mut storage, &mut light_controller, rmk_config),
                    };
                    let mut tasks = vec![devices_task, rmk_task, keyboard_task];
                    if !processors.is_empty() {
                        tasks.push(processors_task);
                    };
                    let central_serials = split_config
                        .central
                        .serial
                        .clone()
                        .expect("No serial defined for central");
                    split_config.peripheral.iter().enumerate().for_each(|(idx, p)| {
                        let row = p.rows;
                        let col = p.cols;
                        let row_offset = p.row_offset;
                        let col_offset = p.col_offset;
                        let uart_instance = format_ident!(
                            "{}",
                            central_serials
                                .get(idx)
                                .expect("No or not enough serial defined for peripheral in central")
                                .instance
                                .to_lowercase()
                        );
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
                        ::rmk::run_rmk(&keymap, driver, &stack, &mut storage, &mut light_controller, rmk_config),
                    };
                    let mut tasks = vec![devices_task, rmk_task, keyboard_task];
                    if !processors.is_empty() {
                        tasks.push(processors_task);
                    };
                    split_config.peripheral.iter().enumerate().for_each(|(idx, p)| {
                        let row = p.rows;
                        let col = p.cols;
                        let row_offset = p.row_offset;
                        let col_offset = p.col_offset;
                        tasks.push(quote! {
                            ::rmk::split::central::run_peripheral_manager::<#row, #col, #row_offset, #col_offset, _>(
                                #idx,
                                peripheral_addrs[#idx],
                                &stack,
                            )
                        });
                    });
                    join_all_tasks(tasks)
                }
                ChipSeries::Esp32 => panic!("Split for esp32 isn't implemented yet"),
            }
        }
        BoardConfig::UniBody(_) => rmk_entry_default(keyboard_config, devices_task, processors_task),
    };
    quote! {
        use ::rmk::input_device::Runnable;
        #entry
    }
}

pub(crate) fn rmk_entry_default(
    keyboard_config: &KeyboardConfig,
    devices_task: TokenStream2,
    processors_task: TokenStream2,
) -> TokenStream2 {
    let keyboard_task = quote! {
        keyboard.run()
    };

    let mut tasks = vec![devices_task, keyboard_task];
    if !processors_task.is_empty() {
        tasks.push(processors_task);
    }
    match keyboard_config.chip.series {
        ChipSeries::Nrf52 => match keyboard_config.communication {
            CommunicationConfig::Usb(_) => {
                let rmk_task = quote! {
                    ::rmk::run_rmk(&keymap, driver, &mut storage, &mut light_controller, rmk_config)
                };
                tasks.push(rmk_task);
                join_all_tasks(tasks)
            }
            CommunicationConfig::Ble(_) => {
                let rmk_task = quote! {
                    ::rmk::run_rmk(&keymap, &stack, &mut storage, &mut light_controller, rmk_config)
                };
                tasks.push(rmk_task);
                join_all_tasks(tasks)
            }
            CommunicationConfig::Both(_, _) => {
                let rmk_task = quote! {
                    ::rmk::run_rmk(&keymap, driver, &stack, &mut storage, &mut light_controller, rmk_config)
                };
                tasks.push(rmk_task);
                join_all_tasks(tasks)
            }
            CommunicationConfig::None => quote! {},
        },
        ChipSeries::Esp32 => {
            let rmk_task = quote! {
                ::rmk::run_rmk(&keymap, &stack, &mut storage, &mut light_controller, rmk_config),
            };
            tasks.push(rmk_task);
            join_all_tasks(tasks)
        }
        _ => {
            let rmk_task = quote! {
                ::rmk::run_rmk(&keymap, driver, &mut storage, &mut light_controller, rmk_config)
            };
            tasks.push(rmk_task);
            join_all_tasks(tasks)
        }
    }
}

pub fn expand_tasks(tasks: Vec<TokenStream2>) -> TokenStream2 {
    let mut current_joined = quote! {};
    tasks.iter().enumerate().for_each(|(id, task)| {
        if id == 0 {
            current_joined = quote! {#task};
        } else {
            current_joined = quote! {
                ::rmk::embassy_futures::join::join(#current_joined, #task)
            };
        }
    });
    current_joined
}

pub(crate) fn join_all_tasks(tasks: Vec<TokenStream2>) -> TokenStream2 {
    let joined = expand_tasks(tasks);
    quote! {
        #joined.await;
    }
}
