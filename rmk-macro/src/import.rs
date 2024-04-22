//! Initialize imports boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ItemMod;

pub(crate) fn expand_imports(item_mod: &ItemMod) -> TokenStream2 {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        let imports = items.iter().map(|item| {
            if let syn::Item::Use(item_use) = &item {
                Some(quote! {
                    #item_use
                })
            } else {
                None
            }
        });
        quote! {
            #(#imports)*
        }
    } else {
        TokenStream2::new()
    }
}
