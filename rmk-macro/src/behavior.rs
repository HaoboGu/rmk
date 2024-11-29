//! Initialize behavior config boilerplate of RMK
//!

use quote::quote;
use crate::config::{OneShotConfig, TapHoldConfig, TriLayerConfig};

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
    let default = quote! {::rmk::config::OneShotConfig::default()};
    match one_shot {
        Some(one_shot) => {
            let millis = match &one_shot.timeout {
                Some(t) => t.0,
                None => return default,
            };

            let timeout = quote! {::embassy_time::Duration::from_millis(#millis)};

            quote! {
                ::rmk::config::OneShotConfig {
                    timeout: #timeout,
                }
            }
        }
        None => default,
    }
}

fn expand_tap_hold(tap_hold: &Option<TapHoldConfig>) -> proc_macro2::TokenStream {
    let default = quote! {::rmk::config::keyboard_config::TapHoldConfig::default()};
    match tap_hold {
        Some(tap_hold) => {
            let enable_hrm = tap_hold.enable_hrm.unwrap_or_default();
            let prior_idle_time = match &tap_hold.prior_idle_time {
                Some(t) => t.0,
                None => 120,
            };
            let post_wait_time = match &tap_hold.post_wait_time {
                Some(t) => t.0,
                None => 50,
            };
            let hold_timeout = match &tap_hold.hold_timeout {
                Some(t) => t.0,
                None => 250,
            };

            quote! {
                ::rmk::config::keyboard_config::TapHoldConfig {
                    enable_hrm: #enable_hrm,
                    prior_idle_time: ::embassy_time::Duration::from_millis(#prior_idle_time),
                    post_wait_time: ::embassy_time::Duration::from_millis(#post_wait_time),
                    hold_timeout: ::embassy_time::Duration::from_millis(#hold_timeout),
                }
            }
        }
        None => default,
    }
}

pub(crate) fn expand_behavior_config(keyboard_config: &KeyboardConfig) -> proc_macro2::TokenStream {
    let tri_layer = expand_tri_layer(&keyboard_config.behavior.tri_layer);
    let tap_hold = expand_tap_hold(&keyboard_config.behavior.tap_hold);
    let one_shot = expand_one_shot(&keyboard_config.behavior.one_shot);

    quote! {
        let behavior_config = ::rmk::config::BehaviorConfig {
            tri_layer: #tri_layer,
            tap_hold: #tap_hold,
            one_shot: #one_shot,
        };
    }
}
