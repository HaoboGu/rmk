//! Initialize default keymap from config
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::LayoutConfig;

pub(crate) fn expand_layout_init(layout_config: Option<LayoutConfig>) -> TokenStream2 {
    // TODO: Generate keymap
    if let Some(l) = layout_config {
        for layer in l.keymap {
            for row in layer {
                for col in row {
                    eprintln!("{}", col)
                }
            }
        }
    };
    quote! {}
}
