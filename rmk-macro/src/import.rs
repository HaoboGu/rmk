//! Initialize imports boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ItemMod;

/// Expand user-customized imports
pub(crate) fn expand_custom_imports(item_mod: &ItemMod) -> TokenStream2 {
    // Parse added imports in mod
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
