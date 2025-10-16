//! Initialize behavior config boilerplate of RMK
//!
use std::collections::HashMap;

use quote::quote;
use rmk_config::{
    CombosConfig, ForksConfig, KeyboardTomlConfig, MacrosConfig, MorseActionPair, MorseConfig, MorseProfile,
    MorsesConfig, OneShotConfig, TriLayerConfig,
};

use crate::layout::{get_key_with_alias, parse_key};

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

fn expand_morse_action_pair(
    action_pair: &MorseActionPair,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> proc_macro2::TokenStream {
    let mut pattern = 0b1u16;
    for ch in action_pair.pattern.chars() {
        match ch {
            '1' => pattern = pattern << 1 | 1,
            '-' => pattern = pattern << 1 | 1,
            '_' => pattern = pattern << 1 | 1,
            '0' => pattern = pattern << 1,
            '.' => pattern = pattern << 1,
            _ => {}
        }
    }
    let action = parse_key(action_pair.action.to_owned(), profiles);
    quote! { (rmk::morse::MorsePattern::from_u16(#pattern), #action.to_action()) }
}

fn expand_morse_actions(
    actions: &Vec<MorseActionPair>,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> proc_macro2::TokenStream {
    if actions.len() > 0 {
        let action_pair_def = actions
            .iter()
            .map(|action_pair| expand_morse_action_pair(action_pair, profiles));
        quote! {
            actions: ::rmk::heapless::LinearMap::from_iter([#(#action_pair_def),*]),
        }
    } else {
        quote! {}
    }
}

fn expand_morse(morse: &Option<MorsesConfig>) -> proc_macro2::TokenStream {
    if let Some(config) = morse {
        let enable_flow_tap = match config.enable_flow_tap {
            Some(enable) => quote! { enable_flow_tap: #enable, },
            None => quote! {},
        };

        let prior_idle_time = match &config.prior_idle_time {
            Some(t) => {
                let timeout = t.0;
                quote! { prior_idle_time: ::embassy_time::Duration::from_millis(#timeout), }
            }
            None => quote! {},
        };

        let default_profile = expand_profile(&MorseProfile {
            unilateral_tap: config.unilateral_tap,
            permissive_hold: config.permissive_hold,
            hold_on_other_press: config.hold_on_other_press,
            normal_mode: config.normal_mode,
            hold_timeout: config.hold_timeout.clone(),
            gap_timeout: config.gap_timeout.clone(),
        });

        let morses = match &config.morses {
            Some(morses) => expand_morses(morses, &config.profiles),
            None => quote! {},
        };

        quote! {
            ::rmk::config::MorsesConfig {
                #enable_flow_tap
                #prior_idle_time
                default_profile: #default_profile,
                #morses
                ..Default::default()
            }
        }
    } else {
        quote! { ::rmk::config::MorsesConfig::default() }
    }
}

fn expand_profile(profile: &MorseProfile) -> proc_macro2::TokenStream {
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
                "\n❌ `{:?}` profile name is not found in behavior.morse.profiles",
                profile_name
            );
        }
    } else {
        panic!(
            "\n❌ behavior.morse.profiles is missing, so `{:?}` profile name is not found",
            profile_name
        );
    }
}

fn expand_combos(
    combos: &Option<CombosConfig>,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> proc_macro2::TokenStream {
    let default = quote! { ::core::default::Default::default() };
    match combos {
        Some(combos) => {
            let combos_def = combos.combos.iter().map(|combo| {
                let actions = combo.actions.iter().map(|a| parse_key(a.to_owned(), profiles));
                let output = parse_key(combo.output.to_owned(), profiles);
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
                    combos: {
                        let v = [#(#combos_def),*];
                        core::array::from_fn(|i| {
                            if i < v.len() {
                                Some(v[i])
                            } else {
                                None
                            }
                        })
                    },
                    #timeout
                    ..Default::default()
                }
            }
        }
        None => default,
    }
}

fn expand_macros(macros: &Option<MacrosConfig>) -> proc_macro2::TokenStream {
    let default = quote! { ::core::default::Default::default() };

    match macros {
        Some(macros) => {
            let macros_def = macros.macros.iter().map(|m| {
                let operations = m.operations.iter().map(|op| match op {
                    rmk_config::MacroOperation::Tap { keycode } => {
                        let key = get_key_with_alias(keycode.trim().to_owned());
                        quote! { ::rmk::keyboard_macros::MacroOperation::Tap(::rmk::types::keycode::KeyCode::#key).into_iter() }
                    }
                    rmk_config::MacroOperation::Down { keycode } => {
                        let key = get_key_with_alias(keycode.trim().to_owned());
                        quote! { ::rmk::keyboard_macros::MacroOperation::Press(::rmk::types::keycode::KeyCode::#key).into_iter() }
                    }
                    rmk_config::MacroOperation::Up { keycode } => {
                        let key = get_key_with_alias(keycode.trim().to_owned());
                        quote! { ::rmk::keyboard_macros::MacroOperation::Release(::rmk::types::keycode::KeyCode::#key).into_iter() }
                    }
                    rmk_config::MacroOperation::Delay { duration } => {
                        let millis = duration.0 as u16;
                        quote! { ::rmk::keyboard_macros::MacroOperation::Delay(#millis).into_iter() }
                    }
                    rmk_config::MacroOperation::Text { text } => {
                        quote! { ::rmk::keyboard_macros::to_macro_sequence(#text).into_iter() }
                    }
                });

                quote! { [#(#operations),*].into_iter().flatten().collect() }
            });

            quote! { ::rmk::config::macro_config::KeyboardMacrosConfig::new(::rmk::keyboard_macros::define_macro_sequences(&[#(#macros_def),*])) }
        }
        None => default,
    }
}

fn expand_morses(
    morses: &Vec<MorseConfig>,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> proc_macro2::TokenStream {
    let morses_def = morses.iter().map(|morse| {
        let profile = if let Some(profile_name) = &morse.profile {
            let morse_profile = expand_profile_name(&profile_name, &profiles);
            quote! { #morse_profile }
        } else {
            quote! { rmk::types::action::MorseProfile::const_default() }
        };

        if let Some(morse_actions) = &morse.morse_actions {
            if morse.tap.is_some() || morse.hold.is_some() || morse.hold_after_tap.is_some() || morse.double_tap.is_some() || morse.tap_actions.is_some() || morse.hold_actions.is_some() {
                panic!("\n❌ keyboard.toml: `morse_actions` cannot be used together with `tap_actions`, `hold_actions`, `tap`, `hold`, `hold_after_tap`, or `double_tap`. Please check the documentation: https://rmk.rs/docs/features/configuration/behavior.html#morse");
            }

            let actions_def = expand_morse_actions(&morse_actions, profiles);

            quote! {
                ::rmk::morse::Morse {
                    profile: #profile,
                    #actions_def
                    ..Default::default()
                }
            }

        } else if morse.tap_actions.is_some() || morse.hold_actions.is_some() {
            // Check first
            if morse.tap.is_some() || morse.hold.is_some() || morse.hold_after_tap.is_some() || morse.double_tap.is_some() {
                panic!("\n❌ keyboard.toml: `tap_actions` and `hold_actions` cannot be used together with `tap`, `hold`, `hold_after_tap`, or `double_tap`. Please check the documentation: https://rmk.rs/docs/features/configuration/behavior.html#morse");
            }

            let tap_actions_def = match &morse.tap_actions {
                Some(tap_actions) => {
                    let actions = tap_actions.iter().map(|action| {
                        let parsed_action = parse_key(action.clone(), profiles);
                        quote! { #parsed_action }
                    });
                    quote! { ::rmk::heapless::Vec::from_iter([#(#actions.to_action()),*]) }
                }
                None => quote! { ::rmk::heapless::Vec::new() },
            };

            let hold_actions_def = match &morse.hold_actions {
                Some(hold_actions) => {
                    let actions = hold_actions.iter().map(|action| {
                        let parsed_action = parse_key(action.clone(), profiles);
                        quote! { #parsed_action }
                    });
                    quote! { ::rmk::heapless::Vec::from_iter([#(#actions.to_action()),*]) }
                }
                None => quote! { ::rmk::heapless::Vec::new() },
            };

            quote! {
                ::rmk::morse::Morse::new_with_actions(
                    #tap_actions_def,
                    #hold_actions_def,
                    #profile,
                )
            }
        } else {
            let tap = parse_key(morse.tap.clone().unwrap_or_else(|| "No".to_string()), profiles);
            let hold = parse_key(morse.hold.clone().unwrap_or_else(|| "No".to_string()), profiles);
            let hold_after_tap = parse_key(morse.hold_after_tap.clone().unwrap_or_else(|| "No".to_string()), profiles);
            let double_tap = parse_key(morse.double_tap.clone().unwrap_or_else(|| "No".to_string()), profiles);

            quote! {
                ::rmk::morse::Morse::new_from_vial(
                    #tap.to_action(),
                    #hold.to_action(),
                    #hold_after_tap.to_action(),
                    #double_tap.to_action(),
                    #profile,
                )
            }
        }
    });

    quote! { morses: ::rmk::heapless::Vec::from_iter([#(#morses_def),*]), }
}

#[derive(PartialEq, Eq, Default)]
struct StateBitsMacro {
    modifiers_left_ctrl: bool,
    modifiers_left_shift: bool,
    modifiers_left_alt: bool,
    modifiers_left_gui: bool,
    modifiers_right_ctrl: bool,
    modifiers_right_shift: bool,
    modifiers_right_alt: bool,
    modifiers_right_gui: bool,

    leds_num_lock: bool,
    leds_caps_lock: bool,
    leds_scroll_lock: bool,
    leds_compose: bool,
    leds_kana: bool,

    mouse_button1: bool,
    mouse_button2: bool,
    mouse_button3: bool,
    mouse_button4: bool,
    mouse_button5: bool,
    mouse_button6: bool,
    mouse_button7: bool,
    mouse_button8: bool,
}

impl StateBitsMacro {
    fn is_empty(&self) -> bool {
        !(self.modifiers_left_ctrl
            || self.modifiers_left_shift
            || self.modifiers_left_alt
            || self.modifiers_left_gui
            || self.modifiers_right_ctrl
            || self.modifiers_right_shift
            || self.modifiers_right_alt
            || self.modifiers_right_gui
            || self.leds_num_lock
            || self.leds_caps_lock
            || self.leds_scroll_lock
            || self.leds_compose
            || self.leds_kana
            || self.mouse_button1
            || self.mouse_button2
            || self.mouse_button3
            || self.mouse_button4
            || self.mouse_button5
            || self.mouse_button6
            || self.mouse_button7
            || self.mouse_button8)
    }
}
// Allows to use `#modifiers` in the quote
impl quote::ToTokens for StateBitsMacro {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let left_ctrl = self.modifiers_left_ctrl;
        let left_shift = self.modifiers_left_shift;
        let left_alt = self.modifiers_left_alt;
        let left_gui = self.modifiers_left_gui;
        let right_ctrl = self.modifiers_right_ctrl;
        let right_shift = self.modifiers_right_shift;
        let right_alt = self.modifiers_right_alt;
        let right_gui = self.modifiers_right_gui;

        let num_lock = self.leds_num_lock;
        let caps_lock = self.leds_caps_lock;
        let scroll_lock = self.leds_scroll_lock;
        let compose = self.leds_compose;
        let kana = self.leds_kana;

        let button1 = self.mouse_button1;
        let button2 = self.mouse_button2;
        let button3 = self.mouse_button3;
        let button4 = self.mouse_button4;
        let button5 = self.mouse_button5;
        let button6 = self.mouse_button6;
        let button7 = self.mouse_button7;
        let button8 = self.mouse_button8;

        tokens.extend(quote! {
            ::rmk::fork::StateBits::new_from(
                ::rmk::types::modifier::ModifierCombination::new_from_vals(#left_ctrl, #left_shift, #left_alt, #left_gui, #right_ctrl, #right_shift, #right_alt, #right_gui),
                ::rmk::types::led_indicator::LedIndicator::new_from(#num_lock, #caps_lock, #scroll_lock, #compose, #kana),
                ::rmk::types::mouse_button::MouseButtons::new_from(#button1, #button2, #button3, #button4, #button5, #button6, #button7, #button8))
        });
    }
}

/// Get modifier combination, in types of mod1 | mod2 | ...
fn parse_state_combination(states_str: &str) -> StateBitsMacro {
    let mut combination = StateBitsMacro::default();
    let tokens = states_str.split_terminator("|");
    tokens.for_each(|w| {
        let w = w.trim();
        match w {
            "LCtrl" => combination.modifiers_left_ctrl = true,
            "LShift" => combination.modifiers_left_shift = true,
            "LAlt" => combination.modifiers_left_alt = true,
            "LGui" => combination.modifiers_left_gui = true,
            "RCtrl" => combination.modifiers_right_ctrl = true,
            "RShift" => combination.modifiers_right_shift = true,
            "RAlt" => combination.modifiers_right_alt = true,
            "RGui" => combination.modifiers_right_gui = true,

            "NumLock" => combination.leds_num_lock = true,
            "CapsLock" => combination.leds_caps_lock = true,
            "ScrollLock" => combination.leds_scroll_lock = true,
            "Compose" => combination.leds_compose = true,
            "Kana" => combination.leds_kana = true,

            "MouseBtn1" => combination.mouse_button1 = true,
            "MouseBtn2" => combination.mouse_button2 = true,
            "MouseBtn3" => combination.mouse_button3 = true,
            "MouseBtn4" => combination.mouse_button4 = true,
            "MouseBtn5" => combination.mouse_button5 = true,
            "MouseBtn6" => combination.mouse_button6 = true,
            "MouseBtn7" => combination.mouse_button7 = true,
            "MouseBtn8" => combination.mouse_button8 = true,
            _ => (),
        }
    });

    combination
}

fn expand_forks(
    forks: &Option<ForksConfig>,
    profiles: &Option<HashMap<String, MorseProfile>>,
) -> proc_macro2::TokenStream {
    let default = quote! { ::core::default::Default::default() };
    match forks {
        Some(forks) => {
            let forks_def = forks.forks.iter().map(|fork| {
                let trigger = parse_key(fork.trigger.to_owned(), profiles);
                let negative_output = parse_key(fork.negative_output.to_owned(), profiles);
                let positive_output = parse_key(fork.positive_output.to_owned(), profiles);
                let match_any  = fork.match_any.as_ref().map(|s| parse_state_combination(s)).unwrap_or_default();
                let match_none = fork.match_none.as_ref().map(|s| parse_state_combination(s)).unwrap_or_default();
                let kept = fork.kept_modifiers.as_ref().map(|s| parse_state_combination(s)).unwrap_or_default();
                let bindable = fork.bindable.unwrap_or(false);

                if match_any.is_empty() && match_none.is_empty() {
                    panic!("\n❌ keyboard.toml: fork configuration missing match conditions! Please check the documentation: https://rmk.rs/docs/features/configuration/behavior.html#fork");
                }

                quote! { ::rmk::fork::Fork::new_ex(#trigger, #negative_output, #positive_output, #match_any, #match_none, #kept, #bindable) }
            });

            quote! {
                ::rmk::config::ForksConfig {
                    forks: ::rmk::heapless::Vec::from_iter([#(#forks_def),*]),
                    ..Default::default()
                }
            }
        }
        None => default,
    }
}

pub(crate) fn expand_behavior_config(keyboard_config: &KeyboardTomlConfig) -> proc_macro2::TokenStream {
    let profiles = &keyboard_config
        .get_behavior_config()
        .unwrap()
        .morse
        .and_then(|m| m.profiles);
    let behavior = keyboard_config.get_behavior_config().unwrap();
    let tri_layer = expand_tri_layer(&behavior.tri_layer);
    let one_shot = expand_one_shot(&behavior.one_shot);
    let combos = expand_combos(&behavior.combo, profiles);
    let macros = expand_macros(&behavior.macros);
    let forks = expand_forks(&behavior.fork, profiles);
    let morse = expand_morse(&behavior.morse);

    quote! {
        let mut behavior_config = ::rmk::config::BehaviorConfig {
            tri_layer: #tri_layer,
            one_shot: #one_shot,
            combo: #combos,
            fork: #forks,
            morse: #morse,
            keyboard_macros: #macros,
            mouse_key: ::rmk::config::MouseKeyConfig::default(),
            tap: ::rmk::config::TapConfig::default(),
        };
    }
}
