//! Initialize behavior config boilerplate of RMK
//!

use quote::quote;
use rmk_config::toml_config::TriLayerConfig;

use crate::keyboard_config::KeyboardConfig;

fn expand_tri_layer(tri_layer: &Option<TriLayerConfig>) -> proc_macro2::TokenStream {
    match tri_layer {
        Some(tri_layer) => {
            let upper = tri_layer.upper;
            let lower = tri_layer.lower;
            let adjust = tri_layer.adjust;
            quote! {::core::option::Option::Some([#upper, #lower, #adjust])}
        }
        None => quote! {::core::option::Option::None::<[u8; 3]>},
    }
}

pub(crate) fn expand_behavior_config(keyboard_config: &KeyboardConfig) -> proc_macro2::TokenStream {
    let tri_layer = expand_tri_layer(&keyboard_config.behavior.tri_layer);
    let hrm = keyboard_config.behavior.enable_hrm.unwrap_or(false);

    quote! {
        let behavior_config = ::rmk::config::keyboard_config::BehaviorConfig {
            tri_layer: #tri_layer,
            enable_hrm: #hrm,
        };
    }
}
