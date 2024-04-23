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
    let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
    let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
    match chip.series {
        crate::ChipSeries::Stm32 => {
            let usb_mod_name = if usb_info.peripheral_name.contains("OTG") {
                format_ident!("{}", "usb_otg")
            } else {
                format_ident!("{}", "usb")
            };
            quote! {
                use ::embassy_stm32::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    #interrupt_name => ::embassy_stm32::#usb_mod_name::InterruptHandler<::embassy_stm32::peripherals::#peripheral_name>;
                });
            }
        }
        crate::ChipSeries::Nrf52 => {
            if chip.has_usb() {
                quote! {
                    use ::embassy_nrf::bind_interrupts;
                    bind_interrupts!(struct Irqs {
                        #interrupt_name => ::embassy_nrf::usb::InterruptHandler<::embassy_nrf::peripherals::#peripheral_name>;
                        POWER_CLOCK => ::embassy_nrf::usb::vbus_detect::InterruptHandler;
                    });
                }
            } else {
                quote! {}
            }
        }
        crate::ChipSeries::Rp2040 => {
            quote! {
                use ::embassy_rp::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    #interrupt_name => ::embassy_rp::usb::InterruptHandler<::embassy_rp::peripherals::#peripheral_name>;
                });
            }
        }
        crate::ChipSeries::Esp32 => quote! {},
        crate::ChipSeries::Unsupported => quote! {},
    }
}
