//! Initialize default keymap from config

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use rmk_config::{KeyboardTomlConfig, KEYCODE_ALIAS};

/// Read the default keymap setting in `keyboard.toml` and add as a `get_default_keymap` function
pub(crate) fn expand_default_keymap(keyboard_config: &KeyboardTomlConfig) -> TokenStream2 {
    let num_encoder = keyboard_config.get_board_config().unwrap().get_num_encoder();
    let total_num_encoder = num_encoder.iter().sum::<usize>();
    // TODO: config encoder in keyboard.toml
    let encoders = vec![quote! { ::rmk::encoder!(::rmk::k!(No), ::rmk::k!(No))}; total_num_encoder];

    let mut layers = vec![];
    let mut encoder_map = vec![];
    for layer in keyboard_config.get_layout_config().unwrap().keymap {
        layers.push(expand_layer(layer));
        encoder_map.push(quote! { [#(#encoders), *] });
    }

    quote! {
        pub const fn get_default_keymap() -> [[[::rmk::action::KeyAction; COL]; ROW]; NUM_LAYER] {
            [#(#layers), *]
        }

        pub const fn get_default_encoder_map() -> [[::rmk::action::EncoderAction; NUM_ENCODER]; NUM_LAYER] {
            [#(#encoder_map), *]
        }
    }
}

/// Push rows in the layer
fn expand_layer(layer: Vec<Vec<String>>) -> TokenStream2 {
    let mut rows = vec![];
    for row in layer {
        rows.push(expand_row(row));
    }
    quote! { [#(#rows), *] }
}

/// Push keys in the row
fn expand_row(row: Vec<String>) -> TokenStream2 {
    let mut keys = vec![];
    for key in row {
        keys.push(parse_key(key));
    }
    quote! { [#(#keys), *] }
}

struct ModifierCombinationMacro {
    right: bool,
    gui: bool,
    alt: bool,
    shift: bool,
    ctrl: bool,
}
impl ModifierCombinationMacro {
    fn new() -> Self {
        Self {
            right: false,
            gui: false,
            alt: false,
            shift: false,
            ctrl: false,
        }
    }
    fn is_empty(&self) -> bool {
        !(self.gui || self.alt || self.shift || self.ctrl)
    }
}
// Allows to use `#modifiers` in the quote
impl quote::ToTokens for ModifierCombinationMacro {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let right = self.right;
        let gui = self.gui;
        let alt = self.alt;
        let shift = self.shift;
        let ctrl = self.ctrl;

        tokens.extend(quote! {
            ::rmk::keycode::ModifierCombination::new_from(#right, #gui, #alt, #shift, #ctrl)
        });
    }
}

/// Get modifier combination, in types of mod1 | mod2 | ...
fn parse_modifiers(modifiers_str: &str) -> ModifierCombinationMacro {
    let mut combination = ModifierCombinationMacro::new();
    let tokens = modifiers_str.split_terminator("|");
    tokens.for_each(|w| {
        let w = w.trim();
        let key = match KEYCODE_ALIAS.get(w.to_lowercase().as_str()) {
            Some(k) => *k,
            None => w,
        };
        match key {
            "LShift" => combination.shift = true,
            "LCtrl" => combination.ctrl = true,
            "LAlt" => combination.alt = true,
            "LGui" => combination.gui = true,
            "RShift" => {
                combination.right = true;
                combination.shift = true;
            }
            "RCtrl" => {
                combination.right = true;
                combination.ctrl = true;
            }
            "RAlt" => {
                combination.right = true;
                combination.alt = true;
            }
            "RGui" => {
                combination.right = true;
                combination.gui = true;
            }
            _ => (),
        }
    });
    combination
}

/// Parse the key string at a single position
pub(crate) fn parse_key(key: String) -> TokenStream2 {
    if !key.is_empty() && (key.trim_start_matches("_").is_empty() || key.to_lowercase() == "trns") {
        return quote! { ::rmk::a!(Transparent) };
    } else if !key.is_empty() && key == "No" {
        return quote! { ::rmk::a!(No) };
    }

    match key {
        s if s.to_lowercase().starts_with("wm(") => {
            let prefix = s.get(0..3).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                let keys: Vec<&str> = internal
                    .split_terminator(",")
                    .map(|w| w.trim())
                    .filter(|w| !w.is_empty())
                    .collect();
                if keys.len() != 2 {
                    panic!("\n❌ keyboard.toml: WM(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }

                let ident = get_key_with_alias(keys[0].to_string());

                let modifiers = parse_modifiers(keys[1]);

                if modifiers.is_empty() {
                    panic!("\n❌ keyboard.toml: modifier in WM(layer, modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                quote! {
                    ::rmk::wm!(#ident, #modifiers)
                }
            } else {
                panic!("\n❌ keyboard.toml: WM(layer, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
            }
        }
        s if s.to_lowercase().starts_with("mo(") => {
            let layer = get_layer(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::mo!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("osl(") => {
            let layer = get_layer(s.clone(), s.get(0..4).unwrap(), ")");
            quote! {
                ::rmk::osl!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("osm(") => {
            let prefix = s.get(0..4).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                let modifiers = parse_modifiers(internal);

                if modifiers.is_empty() {
                    panic!("\n❌ keyboard.toml: modifier in OSM(modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                quote! {
                    ::rmk::osm!(#modifiers)
                }
            } else {
                panic!("\n❌ keyboard.toml: OSM(modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
            }
        }
        s if s.to_lowercase().starts_with("lm(") => {
            let prefix = s.get(0..3).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                let keys: Vec<&str> = internal
                    .split_terminator(",")
                    .map(|w| w.trim())
                    .filter(|w| !w.is_empty())
                    .collect();
                if keys.len() != 2 {
                    panic!("\n❌ keyboard.toml: LM(layer, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                let layer = keys[0].parse::<u8>().unwrap();

                let modifiers = parse_modifiers(keys[1]);

                if modifiers.is_empty() {
                    panic!("\n❌ keyboard.toml: modifier in LM(layer, modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                quote! {
                    ::rmk::lm!(#layer, #modifiers)
                }
            } else {
                panic!("\n❌ keyboard.toml: LM(layer, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
            }
        }
        s if s.to_lowercase().starts_with("lt(") => {
            let prefix = s.get(0..3).unwrap();
            let keys: Vec<&str> = s
                .trim_start_matches(prefix)
                .trim_end_matches(")")
                .split_terminator(",")
                .map(|w| w.trim())
                .filter(|w| !w.is_empty())
                .collect();
            if keys.len() != 2 {
                panic!("\n❌ keyboard.toml: LT(layer, key) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
            }
            let layer = keys[0].parse::<u8>().unwrap();
            let key = get_key_with_alias(keys[1].to_string());
            quote! {
                ::rmk::lt!(#layer, #key)
            }
        }
        s if s.to_lowercase().starts_with("tt(") => {
            let layer = get_layer(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::tt!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("tg(") => {
            let layer = get_layer(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::tg!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("to(") => {
            let layer = get_layer(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::to!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("df(") => {
            let layer = get_layer(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::df!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("mt(") => {
            let prefix = s.get(0..3).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                let keys: Vec<&str> = internal
                    .split_terminator(",")
                    .map(|w| w.trim())
                    .filter(|w| !w.is_empty())
                    .collect();
                if keys.len() != 2 {
                    panic!("\n❌ keyboard.toml: MT(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                let ident = get_key_with_alias(keys[0].to_string());
                let modifiers = parse_modifiers(keys[1]);

                if modifiers.is_empty() {
                    panic!("\n❌ keyboard.toml: modifier in MT(key, modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                quote! {
                    ::rmk::mt!(#ident, #modifiers)
                }
            } else {
                panic!("\n❌ keyboard.toml: MT(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
            }
        }
        s if s.to_lowercase().starts_with("th(") => {
            let prefix = s.get(0..3).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                let keys: Vec<&str> = internal
                    .split_terminator(",")
                    .map(|w| w.trim())
                    .filter(|w| !w.is_empty())
                    .collect();
                if keys.len() != 2 {
                    panic!("\n❌ keyboard.toml: TH(key_tap, key_hold) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                let ident1 = get_key_with_alias(keys[0].to_string());
                let ident2 = get_key_with_alias(keys[1].to_string());

                quote! {
                    ::rmk::th!(#ident1, #ident2)
                }
            } else {
                panic!("\n❌ keyboard.toml: TH(key_tap, key_hold) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
            }
        }
        s if s.to_lowercase().starts_with("shifted(") => {
            let prefix = s.get(0..8).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                if internal.is_empty() {
                    panic!("\n❌ keyboard.toml: SHIFTED(key) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
                }
                let key = get_key_with_alias(internal.to_string());
                quote! { ::rmk::shifted!(#key) }
            } else {
                panic!("\n❌ keyboard.toml: SHIFTED(key) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html");
            }
        }
        _ => {
            let ident = get_key_with_alias(key);
            quote! { ::rmk::k!(#ident) }
        }
    }
}

/// Parse the string literal like `MO(1)`, `OSL(1)`, get the layer number in it.
/// The caller should pass the trimmed prefix and suffix
fn get_layer(key: String, prefix: &str, suffix: &str) -> u8 {
    let layer_str = key.trim_start_matches(prefix).trim_end_matches(suffix);
    layer_str.parse::<u8>().unwrap()
}

fn get_key_with_alias(key: String) -> Ident {
    let key = match KEYCODE_ALIAS.get(key.to_lowercase().as_str()) {
        Some(k) => *k,
        None => key.as_str(),
    };
    format_ident!("{}", key)
}
