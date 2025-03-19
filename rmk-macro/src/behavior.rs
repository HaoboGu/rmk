//! Initialize behavior config boilerplate of RMK
//!

use crate::config::{CombosConfig, OneShotConfig, TapHoldConfig, TriLayerConfig};
use crate::keyboard_config::KeyboardConfig;
use crate::layout::parse_key;
use quote::quote;

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
    let default = quote! {::rmk::config::TapHoldConfig::default()};
    match tap_hold {
        Some(tap_hold) => {
            let enable_hrm = match tap_hold.enable_hrm {
                Some(enable) => quote! { enable_hrm: #enable, },
                None => quote! {},
            };
            let prior_idle_time = match &tap_hold.prior_idle_time {
                Some(t) => {
                    let timeout = t.0;
                    quote! { prior_idle_time: ::embassy_time::Duration::from_millis(#timeout), }
                }
                None => quote! {},
            };
            let post_wait_time = match &tap_hold.post_wait_time {
                Some(t) => {
                    let timeout = t.0;
                    quote! { post_wait_time: ::embassy_time::Duration::from_millis(#timeout), }
                }
                None => quote! {},
            };
            let hold_timeout = match &tap_hold.hold_timeout {
                Some(t) => {
                    let timeout = t.0;
                    quote! { hold_timeout: ::embassy_time::Duration::from_millis(#timeout), }
                }
                None => quote! {},
            };

            quote! {
                ::rmk::config::TapHoldConfig {
                    #enable_hrm
                    #prior_idle_time
                    #post_wait_time
                    #hold_timeout
                    ..Default::default()
                }
            }
        }
        None => default,
    }
}

fn expand_combos(combos: &Option<CombosConfig>) -> proc_macro2::TokenStream {
    let default = quote! { ::core::default::Default::default() };
    match combos {
        Some(combos) => {
            let combos_def = combos.combos.iter().map(|combo| {
                let actions = combo.actions.iter().map(|a| parse_key(a.to_owned()));
                let output = parse_key(combo.output.to_owned());
                let layer = match combo.layer {
                    Some(layer) => quote! { ::core::option::Option::Some(#layer) },
                    None => quote! { ::core::option::Option::None },
                };
                quote! { ::rmk::combo::Combo::new([#(#actions),*], #output, #layer) }
            });

            let timeout = match &combos.timeout {
                Some(t) => {
                    let millis = t.0;
                    quote! { timeout: ::embassy_time::Duration::from_millis(#millis), }
                }
                None => quote! {},
            };

            quote! {
                ::rmk::config::CombosConfig {
                    combos: ::rmk::heapless::Vec::from_iter([#(#combos_def),*]),
                    #timeout
                    ..Default::default()
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
    let combos = expand_combos(&keyboard_config.behavior.combo);

    quote! {
        let behavior_config = ::rmk::config::BehaviorConfig {
            tri_layer: #tri_layer,
            tap_hold: #tap_hold,
            one_shot: #one_shot,
            combo: #combos,
        };
    }
}
