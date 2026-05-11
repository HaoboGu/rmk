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
    watchdog_task: Option<TokenStream2>,
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
                watchdog_task,
            ))
    } else {
        rmk_entry_select(
            hardware,
            host,
            devices,
            processors,
            registered_processors,
            watchdog_task,
        )
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
    watchdog_task: Option<TokenStream2>,
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

    let host_service_task = if host.vial_enabled {
        Some(quote! { host_service.run() })
    } else {
        None
    };
    let board = &hardware.board;
    let communication = &hardware.communication;
    let (transport_prelude, mut transport_tasks) = transport_setup(communication, host);

    // Rynk transports are owned by the transport prelude (see
    // `transport_setup`). Their `run` futures live alongside the existing
    // `usb_transport.run()` / `ble_transport.run()` calls in
    // `transport_tasks`, so no extra collection is needed here.
    let _ = &mut transport_tasks;

    let entry = match board {
        BoardConfig::Split(split_config) => {
            let keyboard_task = quote! {
                keyboard.run(),
            };
            let mut tasks = vec![devices_task, keyboard_task];
            tasks.extend(registered_processors);
            if let Some(t) = host_service_task {
                tasks.push(t);
            }
            tasks.extend(transport_tasks);
            if let Some(t) = &watchdog_task {
                tasks.push(t.clone());
            }
            if split_config.connection == "ble" {
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
                let joined = join_all_tasks(tasks);
                quote! {
                    #transport_prelude
                    #joined
                }
            } else if split_config.connection == "serial" {
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
                let joined = join_all_tasks(tasks);
                quote! {
                    #transport_prelude
                    #joined
                }
            } else {
                panic!(
                    "Invalid split connection type: {}, only \"ble\" and \"serial\" are supported",
                    split_config.connection
                );
            }
        }
        BoardConfig::UniBody(_) => rmk_entry_unibody(
            transport_prelude,
            transport_tasks,
            host_service_task,
            devices_task,
            processors_task,
            registered_processors,
            watchdog_task,
        ),
    };

    quote! {
        use ::rmk::core_traits::Runnable;
        #entry
    }
}

pub(crate) fn rmk_entry_unibody(
    transport_prelude: TokenStream2,
    transport_tasks: Vec<TokenStream2>,
    host_service_task: Option<TokenStream2>,
    devices_task: TokenStream2,
    processors_task: TokenStream2,
    registered_processors: Vec<TokenStream2>,
    watchdog_task: Option<TokenStream2>,
) -> TokenStream2 {
    let keyboard_task = quote! {
        keyboard.run()
    };

    let mut tasks = vec![devices_task, keyboard_task];
    if let Some(t) = host_service_task {
        tasks.push(t);
    }
    if !processors_task.is_empty() {
        tasks.push(processors_task);
    }
    tasks.extend(registered_processors);
    tasks.extend(transport_tasks);
    if let Some(t) = watchdog_task {
        tasks.push(t);
    }
    let joined = join_all_tasks(tasks);
    quote! {
        #transport_prelude
        #joined
    }
}

/// Build (`let mut transport = ...;` prelude, transport `.run()` tasks) for the
/// active communication config. The prelude must be emitted before the join so
/// that `transport.run()` can borrow each transport for the lifetime of the
/// program.
///
/// When `host.rynk_enabled`, a Rynk USB transport is built on the same
/// `embassy_usb::Builder` as the keyboard's HID interfaces and joined as
/// an additional future. A Rynk BLE transport is built off the GATT
/// server inside the BLE per-connection runner; the keyboard's
/// `BleTransport::run` future handles that hookup, so nothing extra needs
/// to be added here for BLE.
fn transport_setup(
    communication: &CommunicationConfig,
    host: &Host,
) -> (TokenStream2, Vec<TokenStream2>) {
    let wpm_prelude = quote! {
        let mut wpm_processor = ::rmk::processor::builtin::wpm::WpmProcessor::new();
    };
    let wpm_task = quote! { wpm_processor.run() };

    // When Rynk is on, `UsbTransport::new_with` is used to attach the
    // vendor-class bulk interface to the same USB builder. The returned
    // tuple gives back the rynk transport so its `run` future can join
    // `run_all!`.
    let (usb_init, rynk_usb_task): (TokenStream2, Option<TokenStream2>) = if host.rynk_enabled {
        let init = quote! {
            let (mut usb_transport, mut rynk_usb) = ::rmk::usb::UsbTransport::new_with(
                driver,
                rmk_config.device_config,
                |builder| {
                    ::rmk::host::RynkUsbTransport::build(builder, ::rmk::host::RYNK_USB_MAX_PACKET_SIZE)
                },
            );
        };
        (init, Some(quote! { rynk_usb.run(&rynk_service) }))
    } else {
        (
            quote! {
                let mut usb_transport = ::rmk::usb::UsbTransport::new(driver, rmk_config.device_config);
            },
            None,
        )
    };

    // BLE rynk hookup is conditional on `host.rynk_enabled`, not the
    // Cargo feature — the `RynkService` binding only exists when the
    // macro emitted the host_service_init Rynk branch.
    let ble_rynk_attach: TokenStream2 = if host.rynk_enabled {
        quote! { let mut ble_transport = ble_transport.with_rynk_service(&rynk_service); }
    } else {
        quote! {}
    };

    match communication {
        CommunicationConfig::Usb(_) => {
            let prelude = quote! {
                #wpm_prelude
                #usb_init
            };
            let mut tasks = vec![quote! { usb_transport.run() }, wpm_task];
            if let Some(t) = rynk_usb_task {
                tasks.push(t);
            }
            (prelude, tasks)
        }
        CommunicationConfig::Ble(_) => {
            let prelude = quote! {
                #wpm_prelude
                #[allow(unused_mut)]
                let mut ble_transport = ::rmk::ble::BleTransport::new(&stack, rmk_config).await;
                #ble_rynk_attach
            };
            (prelude, vec![quote! { ble_transport.run() }, wpm_task])
        }
        CommunicationConfig::Both(_, _) => {
            let prelude = quote! {
                #wpm_prelude
                #usb_init
                #[allow(unused_mut)]
                let mut ble_transport = ::rmk::ble::BleTransport::new(&stack, rmk_config).await;
                #ble_rynk_attach
            };
            let mut tasks = vec![
                quote! { usb_transport.run() },
                quote! { ble_transport.run() },
                wpm_task,
            ];
            if let Some(t) = rynk_usb_task {
                tasks.push(t);
            }
            (prelude, tasks)
        }
        CommunicationConfig::None => panic!("USB and BLE are both disabled"),
    }
}

pub(crate) fn expand_tasks(tasks: Vec<TokenStream2>) -> TokenStream2 {
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
