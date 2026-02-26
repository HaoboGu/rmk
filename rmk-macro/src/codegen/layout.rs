//! Initialize default keymap from config
use std::collections::HashMap;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::{KeyboardTomlConfig, MorseProfile};

use super::action_parser::parse_key;

/// Read the default keymap setting in `keyboard.toml` and add as a `get_default_keymap` function
/// Also add `get_default_encoder_map`
pub(crate) fn expand_default_keymap(keyboard_config: &KeyboardTomlConfig) -> TokenStream2 {
    let profiles = &keyboard_config
        .get_behavior_config()
        .unwrap_or_else(|e| panic!("{}", e))
        .morse
        .and_then(|m| m.profiles);
    let num_encoder = keyboard_config
        .get_board_config()
        .unwrap_or_else(|e| panic!("{}", e))
        .get_num_encoder()
        .iter()
        .sum();

    let (layout, _) = keyboard_config.get_layout_config().unwrap_or_else(|e| panic!("{}", e));

    let mut layers = vec![];
    let mut encoder_map = vec![];

    for layer in &layout.keymap {
        layers.push(expand_layer(layer.clone(), profiles));
    }

    for encoder_layer in &layout.encoder_map {
        encoder_map.push(expand_encoder_layer(
            encoder_layer.clone(),
            num_encoder,
            profiles,
        ));
    }
    encoder_map.resize(
        layout.keymap.len(),
        quote! { [::rmk::encoder!(::rmk::k!(No), ::rmk::k!(No)); NUM_ENCODER] },
    );

    quote! {
        pub const fn get_default_keymap() -> [[[::rmk::types::action::KeyAction; COL]; ROW]; NUM_LAYER] {
            [#(#layers), *]
        }

        pub const fn get_default_encoder_map() -> [[::rmk::types::action::EncoderAction; NUM_ENCODER]; NUM_LAYER] {
            [#(#encoder_map), *]
        }
    }
}

/// Expand a layer for keymap
fn expand_layer(
    layer: Vec<Vec<String>>,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> TokenStream2 {
    let mut rows = vec![];
    for row in layer {
        rows.push(expand_row(row, profiles));
    }
    quote! { [#(#rows), *] }
}

/// Expand a row for keymap
fn expand_row(row: Vec<String>, profiles: &Option<HashMap<String, MorseProfile>>) -> TokenStream2 {
    let mut keys = vec![];
    for key in row {
        keys.push(parse_key(key.clone(), profiles).unwrap_or_else(|e| panic!("failed to parse key action '{}': {}", key, e)));
    }
    quote! { [#(#keys), *] }
}

/// Expand a layer for encoder map
fn expand_encoder_layer(
    encoder_layer: Vec<[String; 2]>,
    num_encoder: usize,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> TokenStream2 {
    let mut encoders = vec![];

    for encoder in encoder_layer {
        let cw_action = parse_key(encoder[0].clone(), profiles).unwrap_or_else(|e| panic!("failed to parse encoder cw action '{}': {}", encoder[0], e));
        let ccw_action = parse_key(encoder[1].clone(), profiles).unwrap_or_else(|e| panic!("failed to parse encoder ccw action '{}': {}", encoder[1], e));
        encoders.push(quote! { ::rmk::encoder!(#cw_action, #ccw_action) });
    }

    // Make sure it configures correct number of encoders
    encoders.resize(
        num_encoder,
        quote! { ::rmk::encoder!(::rmk::k!(No), ::rmk::k!(No)) },
    );

    quote! { [#(#encoders), *] }
}
