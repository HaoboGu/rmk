//! Add `bind_interrupts!` boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::ItemMod;

use crate::{usb_interrupt_map::UsbInfo, ChipModel};

// Expand `bind_interrupt!` stuffs
pub(crate) fn expand_bind_interrupt(
    chip: &ChipModel,
    usb_info: &UsbInfo,
    item_mod: &ItemMod,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(bind_interrupt)]`, override it
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Some(i) = &item_fn.attrs[0].meta.path().get_ident() {
                            if i.to_string() == "bind_interrupt" {
                                let content = &item_fn.block.stmts;
                                return Some(quote! {
                                    #(#content)*
                                });
                            }
                        }
                    }
                }
                None
            })
            .unwrap_or(bind_interrupt_default(chip, usb_info))
    } else {
        bind_interrupt_default(chip, usb_info)
    }
}

pub(crate) fn bind_interrupt_default(chip: &ChipModel, usb_info: &UsbInfo) -> TokenStream2 {
    if !chip.has_usb() {
        return quote! {};
    }
    let (usb_mod_path, chip_mod_path) = match chip.series {
        crate::ChipSeries::Stm32 => {
            let usb_mod_name = if usb_info.peripheral_name.contains("OTG") {
                format_ident!("{}", "usb_otg")
            } else {
                format_ident!("{}", "usb")
            };
            let usb_mod_path = quote! {
                ::embassy_stm32::#usb_mod_name
            };
            let peripheral_mod_path = quote! {
                ::embassy_stm32
            };
            (usb_mod_path, peripheral_mod_path)
        }
        crate::ChipSeries::Nrf52 => {
            if chip.has_usb() {
                let usb_mod_path = quote! {
                    ::embassy_nrf::usb
                };
                let peripheral_mod_path = quote! {
                    ::embassy_nrf
                };
                (usb_mod_path, peripheral_mod_path)
            } else {
                (quote! {}, quote! {})
            }
        }
        crate::ChipSeries::Rp2040 => (
            quote! {
                ::embassy_rp::usb
            },
            quote! {
                ::embassy_rp
            },
        ),
        crate::ChipSeries::Esp32 => (quote! {}, quote! {}),
        crate::ChipSeries::Unsupported => (quote! {}, quote! {}),
    };

    let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
    let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
    quote! {
        use #chip_mod_path::bind_interrupts;

        bind_interrupts!(struct Irqs {
            #interrupt_name => #usb_mod_path::InterruptHandler<#chip_mod_path::peripherals::#peripheral_name>;
        });
    }
}
