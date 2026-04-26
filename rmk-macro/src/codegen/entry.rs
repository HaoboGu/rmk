use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::resolved::hardware::{BoardConfig, CommunicationConfig};
use rmk_config::resolved::{Hardware, Host};
use syn::{ItemFn, ItemMod};

use super::override_helper::Overwritten;

pub(crate) fn expand_rmk_entry(
    hardware: &Hardware,
    host: &Host,
    item_mod: &ItemMod,
    devices: Vec<TokenStream2>,
    processors: Vec<TokenStream2>,
    registered_processors: Vec<TokenStream2>,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(entry)]`, override the entry
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item
                    && item_fn.attrs.len() == 1
                    && let Ok(Overwritten::Entry) = Overwritten::from_meta(&item_fn.attrs[0].meta)
                {
                    return Some(override_rmk_entry(item_fn));
                }
                None
            })
            .unwrap_or(rmk_entry_select(
                hardware,
                host,
                devices,
                processors,
                registered_processors,
            ))
    } else {
        rmk_entry_select(hardware, host, devices, processors, registered_processors)
    }
}

fn override_rmk_entry(item_fn: &ItemFn) -> TokenStream2 {
    let content = &item_fn.block.stmts;
    quote! {
        #(#content)*
    }
}

pub(crate) fn rmk_entry_select(
    hardware: &Hardware,
    host: &Host,
    devices: Vec<TokenStream2>,
    processors: Vec<TokenStream2>,
    registered_processors: Vec<TokenStream2>,
) -> TokenStream2 {
    let devices_task = {
        let mut devs = devices.clone();
        devs.push(quote! {matrix});
        if hardware.storage.is_some() {
            devs.push(quote! {storage});
        }
        quote! {
            ::rmk::run_all! (
                #(#devs),*
            )
        }
    };
    let processors_task = if processors.is_empty() {
        quote! {}
    } else {
        quote! {
            ::rmk::run_all! (
                #(#processors),*
            )
        }
    };

    let keymap = if host.vial_enabled {
        quote! { &keymap, }
    } else {
        quote! {}
    };
    let board = &hardware.board;
    let communication = &hardware.communication;
    let usb_driver_arg = match communication {
        CommunicationConfig::Usb(_) | CommunicationConfig::Both(_, _) => quote! { driver, },
        CommunicationConfig::Ble(_) => quote! {},
        CommunicationConfig::None => panic!("USB and BLE are both disabled"),
    };

    let entry = match board {
        BoardConfig::Split(split_config) => {
            let keyboard_task = quote! {
                keyboard.run(),
            };
            let mut tasks = vec![devices_task, keyboard_task];
            tasks.extend(registered_processors);
            if split_config.connection == "ble" {
                let rmk_task = quote! {
                    ::rmk::run_rmk(#keymap #usb_driver_arg &stack, rmk_config)
                };
                tasks.push(rmk_task);
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
                            &peripheral_addrs,
                            &stack,
                        )
                    });
                });
                let scan_task = quote! {
                    ::rmk::split::ble::central::scan_peripherals(&stack, &peripheral_addrs)
                };
                tasks.push(scan_task);
                join_all_tasks(tasks)
            } else if split_config.connection == "serial" {
                let rmk_task = quote! {
                    ::rmk::run_rmk(#keymap #usb_driver_arg rmk_config),
                };
                tasks.push(rmk_task);
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
            } else {
                panic!(
                    "Invalid split connection type: {}, only \"ble\" and \"serial\" are supported",
                    split_config.connection
                );
            }
        }
        BoardConfig::UniBody(_) => rmk_entry_unibody(
            hardware,
            host,
            devices_task,
            processors_task,
            registered_processors,
        ),
    };

    quote! {
        use ::rmk::core_traits::Runnable;
        #entry
    }
}

pub(crate) fn rmk_entry_unibody(
    hardware: &Hardware,
    host: &Host,
    devices_task: TokenStream2,
    processors_task: TokenStream2,
    registered_processors: Vec<TokenStream2>,
) -> TokenStream2 {
    let keyboard_task = quote! {
        keyboard.run()
    };

    let mut tasks = vec![devices_task, keyboard_task];
    if !processors_task.is_empty() {
        tasks.push(processors_task);
    }
    tasks.extend(registered_processors);
    // Remove the keymap argument if the vial is disabled
    let keymap = if host.vial_enabled {
        quote! { &keymap, }
    } else {
        quote! {}
    };
    let communication = &hardware.communication;
    match communication {
        CommunicationConfig::Usb(_) => {
            let rmk_task = quote! {
                ::rmk::run_rmk(#keymap driver, rmk_config)
            };
            tasks.push(rmk_task);
            join_all_tasks(tasks)
        }
        CommunicationConfig::Ble(_) => {
            let rmk_task = quote! {
                ::rmk::run_rmk(#keymap &stack, rmk_config)
            };
            tasks.push(rmk_task);
            join_all_tasks(tasks)
        }
        CommunicationConfig::Both(_, _) => {
            let rmk_task = quote! {
                ::rmk::run_rmk(#keymap driver, &stack, rmk_config)
            };
            tasks.push(rmk_task);
            join_all_tasks(tasks)
        }
        CommunicationConfig::None => panic!("USB and BLE are both disabled"),
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
