//! Initialize behavior config boilerplate of RMK
//!

use quote::quote;
use rmk_config::toml_config::{OneShotConfig, TriLayerConfig};

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

fn expand_one_shot(one_shot: &Option<OneShotConfig>) -> proc_macro2::TokenStream {
    let default = quote! {::rmk::config::keyboard_config::OneShotConfig::default()};
    match one_shot {
        Some(one_shot) => {
            let millis = match &one_shot.timeout {
                Some(t) => t.0,
                None => return default,
            };

            let timeout = quote! {::embassy_time::Duration::from_millis(#millis)};

            quote! {
                ::rmk::config::keyboard_config::OneShotConfig {
                    timeout: #timeout,
                }
            }
        }
        None => default,
    }
}

pub(crate) fn expand_behavior_config(keyboard_config: &KeyboardConfig) -> proc_macro2::TokenStream {
    let tri_layer = expand_tri_layer(&keyboard_config.behavior.tri_layer);
    let one_shot = expand_one_shot(&keyboard_config.behavior.one_shot);

    quote! {
        let behavior_config = ::rmk::config::keyboard_config::BehaviorConfig {
            tri_layer: #tri_layer,
            one_shot: #one_shot,
        };
    }
}
