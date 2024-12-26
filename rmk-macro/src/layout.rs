//! Initialize default keymap from config
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

use crate::keyboard_config::KeyboardConfig;

/// Read the default keymap setting in `keyboard.toml` and add as a `get_default_keymap` function
pub(crate) fn expand_layout_init(keyboard_config: &KeyboardConfig) -> TokenStream2 {
    let mut layers = vec![];
    for layer in keyboard_config.layout.keymap.clone() {
        layers.push(expand_layer(layer));
    }
    return quote! {
        pub fn get_default_keymap() -> [[[::rmk::action::KeyAction; COL]; ROW]; NUM_LAYER] {
            [#(#layers), *]
        }
    };
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

/// Get modifier combination, in types of mod1 | mod2 | ...
fn parse_modifiers(modifiers_str: &str) -> (bool, bool, bool, bool, bool){
    let mut right = false;
    let mut gui = false;
    let mut alt = false;
    let mut shift = false;
    let mut ctrl = false;
    let tokens = modifiers_str.split_terminator("|");
    tokens.for_each(|w| {
        let w = w.trim();
        match w {
            "LShift" => shift = true,
            "LCtrl" => ctrl = true,
            "LAlt" => alt = true,
            "Lgui" => gui = true,
            "RShift" => {
                right = true;
                shift = true;
            }
            "RCtrl" => {
                right = true;
                ctrl = true;
            }
            "RAlt" => {
                right = true;
                alt = true;
            }
            "Rgui" => {
                right = true;
                gui = true;
            }
            _ => (),
        }
    });
    return (right, gui, alt, shift, ctrl);
}

/// Parse the key string at a single position
fn parse_key(key: String) -> TokenStream2 {
    if key.len() < 5 {
        return if key.len() > 0 && key.trim_start_matches("_").len() == 0 {
            quote! { ::rmk::a!(No) }
        } else {
            let ident = format_ident!("{}", key);
            quote! { ::rmk::k!(#ident) }
        };
    }
    match &key[0..3] {
        "WM(" => {
            if let Some(internal) = key.trim_start_matches("WM(").strip_suffix(")") {
                let keys: Vec<&str> = internal
                    .split_terminator(",")
                    .map(|w| w.trim())
                    .filter(|w| w.len() > 0)
                    .collect();
                if keys.len() != 2 {
                    return quote! {
                        compile_error!("keyboard.toml: WM(layer, modifier) invalid, please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                    };
                }

                let ident = format_ident!("{}", keys[0].to_string());

                let (right, gui, alt, shift, ctrl) = parse_modifiers(keys[1]);

                if (gui || alt || shift || ctrl) == false {
                    return quote! {
                        compile_error!("keyboard.toml: modifier in WM(layer, modifier) is not valid! Please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                    };
                }
                quote! {
                    ::rmk::wm!(#ident, ::rmk::keycode::ModifierCombination::new_from(#right, #gui, #alt, #shift, #ctrl))
                }
            } else {
                return quote! {
                    compile_error!("keyboard.toml: WM(layer, modifier) invalid, please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                };
            }
        }
        "MO(" => {
            let layer = get_layer(key, "MO(", ")");
            quote! {
                ::rmk::mo!(#layer)
            }
        }
        "OSL" => {
            let layer = get_layer(key, "OSL(", ")");
            quote! {
                ::rmk::osl!(#layer)
            }
        }
        "OSM" => {
            if let Some(internal) = key.trim_start_matches("OSM(").strip_suffix(")") {
                let (right, gui, alt, shift, ctrl) = parse_modifiers(internal);

                if !(gui || alt || shift || ctrl) {
                    return quote! {
                        compile_error!("keyboard.toml: modifier in OSM(modifier) is not valid! Please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                    };
                }
                quote! {
                    ::rmk::osm!(::rmk::keycode::ModifierCombination::new_from(#right, #gui, #alt, #shift, #ctrl))
                }
            } else {
                quote! {
                    compile_error!("keyboard.toml: OSM(modifier) invalid, please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                }
            }
        }
        "LM(" => {
            if let Some(internal) = key.trim_start_matches("LM(").strip_suffix(")") {
                let keys: Vec<&str> = internal
                    .split_terminator(",")
                    .map(|w| w.trim())
                    .filter(|w| w.len() > 0)
                    .collect();
                if keys.len() != 2 {
                    return quote! {
                        compile_error!("keyboard.toml: LM(layer, modifier) invalid, please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                    };
                }
                let layer = keys[0].parse::<u8>().unwrap();

                let (right, gui, alt, shift, ctrl) = parse_modifiers(keys[1]);

                if (gui || alt || shift || ctrl) == false {
                    return quote! {
                        compile_error!("keyboard.toml: modifier in LM(layer, modifier) is not valid! Please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                    };
                }
                quote! {
                    ::rmk::lm!(#layer, ::rmk::keycode::ModifierCombination::new_from(#right, #gui, #alt, #shift, #ctrl))
                }
            } else {
                return quote! {
                    compile_error!("keyboard.toml: LM(layer, modifier) invalid, please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                };
            }
        }
        "LT(" => {
            let keys: Vec<&str> = key
                .trim_start_matches("LT(")
                .trim_end_matches(")")
                .split_terminator(",")
                .map(|w| w.trim())
                .filter(|w| w.len() > 0)
                .collect();
            if keys.len() != 2 {
                return quote! {
                    compile_error!("keyboard.toml: LT(layer, key) invalid, please check the documentation: https://haobogu.github.io/rmk/keyboard_configuration.html");
                };
            }
            let layer = keys[0].parse::<u8>().unwrap();
            let key = format_ident!("{}", keys[1]);
            quote! {
                ::rmk::lt!(#layer, #key)
            }
        }
        "TT(" => {
            let layer = get_layer(key, "TT(", ")");
            quote! {
                ::rmk::tt!(#layer)
            }
        }
        "TG(" => {
            let layer = get_layer(key, "TG(", ")");
            quote! {
                ::rmk::tg!(#layer)
            }
        }
        "TO(" => {
            let layer = get_layer(key, "TO(", ")");
            quote! {
                ::rmk::to!(#layer)
            }
        }
        "DF(" => {
            let layer = get_layer(key, "DF(", ")");
            quote! {
                ::rmk::df!(#layer)
            }
        }
        _ => {
            let ident = format_ident!("{}", key);
            quote! {::rmk::k!(#ident) }
        }
    }
}

/// Parse the string literal like `MO(1)`, `OSL(1)`, get the layer number in it.
/// The caller should pass the trimmed prefix and suffix
fn get_layer(key: String, prefix: &str, suffix: &str) -> u8 {
    let layer_str = key.trim_start_matches(prefix).trim_end_matches(suffix);
    layer_str.parse::<u8>().unwrap()
}
