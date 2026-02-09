//! Shared key-parsing and action-expansion helpers.
//!
//! Extracted from `layout.rs` and `behavior.rs` to break the circular
//! dependency between those two modules.

use std::collections::HashMap;

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use rmk_config::{KEYCODE_ALIAS, MorseProfile};
use strum::VariantNames;

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
            ::rmk::types::modifier::ModifierCombination::new_from(#right, #gui, #alt, #shift, #ctrl)
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

pub(crate) fn expand_profile(profile: &MorseProfile) -> proc_macro2::TokenStream {
    let mode = if let Some(enable) = profile.permissive_hold
        && enable
    {
        quote! { ::core::option::Option::Some(rmk::types::action::MorseMode::PermissiveHold) }
    } else if let Some(enable) = profile.hold_on_other_press
        && enable
    {
        quote! { ::core::option::Option::Some(rmk::types::action::MorseMode::HoldOnOtherPress) }
    } else if let Some(enable) = profile.normal_mode
        && enable
    {
        quote! { ::core::option::Option::Some(rmk::types::action::MorseMode::Normal) }
    } else {
        quote! { ::core::option::Option::None }
    };

    let unilateral_tap = if let Some(enable) = profile.unilateral_tap {
        quote! { ::core::option::Option::Some(#enable) }
    } else {
        quote! { ::core::option::Option::None }
    };

    let hold_timeout_ms = match &profile.hold_timeout {
        Some(t) => {
            let timeout = t.0 as u16;
            quote! { ::core::option::Option::Some(#timeout) }
        }
        None => quote! { ::core::option::Option::None },
    };

    let gap_timeout_ms = match &profile.gap_timeout {
        Some(t) => {
            let timeout = t.0 as u16;
            quote! { ::core::option::Option::Some(#timeout) }
        }
        None => quote! { ::core::option::Option::None },
    };

    quote! { rmk::types::action::MorseProfile::new(#unilateral_tap, #mode, #hold_timeout_ms, #gap_timeout_ms) }
}

pub(crate) fn expand_profile_name(
    profile_name: &str,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> proc_macro2::TokenStream {
    if let Some(profiles) = profiles {
        if let Some(profile) = profiles.get(profile_name) {
            let morse_profile = expand_profile(profile);
            quote! { #morse_profile }
        } else {
            panic!(
                "\n\u{274c} `{:?}` profile name is not found in behavior.morse.profiles",
                profile_name
            );
        }
    } else {
        panic!(
            "\n\u{274c} behavior.morse.profiles is missing, so `{:?}` profile name is not found",
            profile_name
        );
    }
}

/// Parse the key string at a single position
pub(crate) fn parse_key(
    key: String,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> TokenStream2 {
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
                    panic!(
                        "\n\u{274c} keyboard.toml: WM(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }

                let ident = get_key_with_alias(keys[0].to_string());

                let modifiers = parse_modifiers(keys[1]);

                if modifiers.is_empty() {
                    panic!(
                        "\n\u{274c} keyboard.toml: modifier in WM(layer, modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                quote! {
                    ::rmk::wm!(#ident, #modifiers)
                }
            } else {
                panic!(
                    "\n\u{274c} keyboard.toml: WM(layer, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                );
            }
        }
        s if s.to_lowercase().starts_with("mo(") => {
            let layer = get_number(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::mo!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("osl(") => {
            let layer = get_number(s.clone(), s.get(0..4).unwrap(), ")");
            quote! {
                ::rmk::osl!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("osm(") => {
            let prefix = s.get(0..4).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                let modifiers = parse_modifiers(internal);

                if modifiers.is_empty() {
                    panic!(
                        "\n\u{274c} keyboard.toml: modifier in OSM(modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                quote! {
                    ::rmk::osm!(#modifiers)
                }
            } else {
                panic!(
                    "\n\u{274c} keyboard.toml: OSM(modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                );
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
                    panic!(
                        "\n\u{274c} keyboard.toml: LM(layer, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                let layer = keys[0].parse::<u8>().unwrap();

                let modifiers = parse_modifiers(keys[1]);

                if modifiers.is_empty() {
                    panic!(
                        "\n\u{274c} keyboard.toml: modifier in LM(layer, modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                quote! {
                    ::rmk::lm!(#layer, #modifiers)
                }
            } else {
                panic!(
                    "\n\u{274c} keyboard.toml: LM(layer, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                );
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
            if keys.len() < 2 || keys.len() > 3 {
                panic!(
                    "\n\u{274c} keyboard.toml: LT(layer, key) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                );
            }
            let layer = keys[0].parse::<u8>().unwrap();
            let key = get_key_with_alias(keys[1].to_string());

            if keys.len() == 3 {
                let profile = expand_profile_name(keys[2], profiles);
                quote! { ::rmk::ltp!(#layer, #key, #profile) }
            } else {
                quote! { ::rmk::lt!(#layer, #key) }
            }
        }
        s if s.to_lowercase().starts_with("tt(") => {
            let layer = get_number(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::tt!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("tg(") => {
            let layer = get_number(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::tg!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("to(") => {
            let layer = get_number(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::to!(#layer)
            }
        }
        s if s.to_lowercase().starts_with("df(") => {
            let layer = get_number(s.clone(), s.get(0..3).unwrap(), ")");
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
                if keys.len() < 2 || keys.len() > 3 {
                    panic!(
                        "\n\u{274c} keyboard.toml: MT(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                let ident = get_key_with_alias(keys[0].to_string());
                let modifiers = parse_modifiers(keys[1]);

                if modifiers.is_empty() {
                    panic!(
                        "\n\u{274c} keyboard.toml: modifier in MT(key, modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                if keys.len() == 3 {
                    let profile = expand_profile_name(keys[2], profiles);
                    quote! { ::rmk::mtp!(#ident, #modifiers, #profile) }
                } else {
                    quote! { ::rmk::mt!(#ident, #modifiers) }
                }
            } else {
                panic!(
                    "\n\u{274c} keyboard.toml: MT(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                );
            }
        }
        s if s.to_lowercase().starts_with("macro(") => {
            let number = get_number(s.clone(), s.get(0..6).unwrap(), ")");
            quote! {
                ::rmk::macros!(#number)
            }
        }
        // s if s.to_lowercase().starts_with("hrm(") => {
        //     let prefix = s.get(0..4).unwrap();
        //     if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
        //         let keys: Vec<&str> = internal
        //             .split_terminator(",")
        //             .map(|w| w.trim())
        //             .filter(|w| !w.is_empty())
        //             .collect();
        //         if keys.len() != 2 {
        //             panic!(
        //                 "\n\u{274c} keyboard.toml: HRM(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
        //             );
        //         }
        //         let ident = get_key_with_alias(keys[0].to_string());
        //         let modifiers = parse_modifiers(keys[1]);

        //         if modifiers.is_empty() {
        //             panic!(
        //                 "\n\u{274c} keyboard.toml: modifier in HRM(key, modifier) is not valid! Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
        //             );
        //         }
        //         quote! {
        //             ::rmk::hrm!(#ident, #modifiers)
        //         }
        //     } else {
        //         panic!(
        //             "\n\u{274c} keyboard.toml: HRM(key, modifier) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
        //         );
        //     }
        // }
        s if s.to_lowercase().starts_with("th(") => {
            let prefix = s.get(0..3).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                let keys: Vec<&str> = internal
                    .split_terminator(",")
                    .map(|w| w.trim())
                    .filter(|w| !w.is_empty())
                    .collect();
                if keys.len() < 2 || keys.len() > 3 {
                    panic!(
                        "\n\u{274c} keyboard.toml: TH(key_tap, key_hold) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                let ident1 = get_key_with_alias(keys[0].to_string());
                let ident2 = get_key_with_alias(keys[1].to_string());

                if keys.len() == 3 {
                    let profile = expand_profile_name(keys[2], profiles);
                    quote! { ::rmk::thp!(#ident1, #ident2, #profile) }
                } else {
                    quote! { ::rmk::th!(#ident1, #ident2) }
                }
            } else {
                panic!(
                    "\n\u{274c} keyboard.toml: TH(key_tap, key_hold) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                );
            }
        }
        s if s.to_lowercase().starts_with("shifted(") => {
            let prefix = s.get(0..8).unwrap();
            if let Some(internal) = s.trim_start_matches(prefix).strip_suffix(")") {
                if internal.is_empty() {
                    panic!(
                        "\n\u{274c} keyboard.toml: SHIFTED(key) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                    );
                }
                let key = get_key_with_alias(internal.to_string());
                quote! { ::rmk::shifted!(#key) }
            } else {
                panic!(
                    "\n\u{274c} keyboard.toml: SHIFTED(key) invalid, please check the documentation: https://rmk.rs/docs/features/configuration/layout.html"
                );
            }
        }
        s if s.to_lowercase().starts_with("td(") => {
            let index = get_number(s.clone(), s.get(0..3).unwrap(), ")");
            quote! {
                ::rmk::td!(#index)
            }
        }
        s if s.to_lowercase().starts_with("user") => {
            // Support both User(X) and UserX formats
            let number_str = if s.to_lowercase().starts_with("user(") {
                // User(X) format
                s.trim_start_matches(|c: char| !c.is_ascii_digit())
                    .trim_end_matches(')')
            } else if s[4..]
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                // UserX format
                &s[4..]
            } else {
                ""
            };

            let number = number_str.parse::<u8>().unwrap_or(255);

            if number > 31 {
                panic!(
                    "\n\u{274c} keyboard.toml: {} is not a valid user key! User keys are numbered 0-31. Please check the documentation: https://rmk.rs/docs/features/configuration/layout.html",
                    s
                );
            }

            quote! {
                ::rmk::user!(#number)
            }
        }
        s if s.to_lowercase().starts_with("macro")
            && s[5..]
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false) =>
        {
            // Support Macro0, Macro1, Macro2, etc.
            let index_str = &s[5..];
            let index = index_str.parse::<u8>().unwrap();
            quote! {
                ::rmk::macros!(#index)
            }
        }
        _ => {
            // Check if it's a keyboard control, light control, or special key action (case-insensitive)
            let key_lower = key.to_lowercase();

            // Try to find exact match (case-insensitive) in keyboard actions
            // Use strum::VariantNames to automatically get all enum variants
            if let Some(action) = rmk_types::action::KeyboardAction::VARIANTS
                .iter()
                .find(|&&a| a.to_lowercase() == key_lower)
            {
                let action_ident = format_ident!("{}", action);
                return quote! { ::rmk::kbctrl!(#action_ident) };
            }

            // Try to find exact match (case-insensitive) in light actions
            if let Some(action) = rmk_types::action::LightAction::VARIANTS
                .iter()
                .find(|&&a| a.to_lowercase() == key_lower)
            {
                let action_ident = format_ident!("{}", action);
                return quote! { ::rmk::light!(#action_ident) };
            }

            // Try to find exact match (case-insensitive) in special keys
            if let Some(special_key) = rmk_types::keycode::SpecialKey::VARIANTS
                .iter()
                .find(|&&k| k.to_lowercase() == key_lower)
            {
                let key_ident = format_ident!("{}", special_key);
                return quote! { ::rmk::special!(#key_ident) };
            }

            // Default: try to use as HID keycode
            let ident = get_key_with_alias(key);
            quote! { ::rmk::k!(#ident) }
        }
    }
}

/// Parse the string literal like `MO(1)`, `OSL(1)`, `TD(0)`, etc, get the number in it.
/// The caller should pass the trimmed prefix and suffix
fn get_number(key: String, prefix: &str, suffix: &str) -> u8 {
    let layer_str = key.trim_start_matches(prefix).trim_end_matches(suffix);
    layer_str.parse::<u8>().unwrap()
}

pub(crate) fn get_key_with_alias(key: String) -> Ident {
    let key = match KEYCODE_ALIAS.get(key.to_lowercase().as_str()) {
        Some(k) => *k,
        None => key.as_str(),
    };
    format_ident!("{}", key)
}
