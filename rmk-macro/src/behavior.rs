//! Initialize behavior config boilerplate of RMK
//!

use quote::quote;
use rmk_config::{
    CombosConfig, ForksConfig, KeyboardTomlConfig, MacrosConfig, OneShotConfig, TapDancesConfig, TapHoldConfig,
    TriLayerConfig,
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

fn expand_morse(morse_config: &Option<TapHoldConfig>) -> proc_macro2::TokenStream {
    let default = quote! {::rmk::config::MorseConfig::default()};
    match morse_config {
        Some(morse) => {
            let enable_hrm = match morse.enable_hrm {
                Some(enable) => quote! { enable_hrm: #enable, },
                None => quote! {},
            };
            let morse_mode = if let Some(true) = morse.permissive_hold {
                quote! { mode: ::rmk::morse::MorseKeyMode::PermissiveHold, }
            } else if let Some(true) = morse.hold_on_other_press {
                quote! { mode: ::rmk::morse::MorseKeyMode::HoldOnOtherPress, }
            } else {
                quote! { mode: ::rmk::morse::MorseKeyMode::Normal,}
            };
            let unilateral_tap = match morse.unilateral_tap {
                Some(enable) => quote! { unilateral_tap: #enable, },
                None => quote! {},
            };
            let prior_idle_time = match &morse.prior_idle_time {
                Some(t) => {
                    let timeout = t.0;
                    quote! { prior_idle_time: ::embassy_time::Duration::from_millis(#timeout), }
                }
                None => quote! {},
            };
            let hold_timeout = match &morse.hold_timeout {
                Some(t) => {
                    let timeout = t.0;
                    quote! { operation_timeout: ::embassy_time::Duration::from_millis(#timeout), }
                }
                None => quote! {},
            };

            quote! {
                ::rmk::config::MorseConfig {
                    #enable_hrm
                    #prior_idle_time
                    #hold_timeout
                    #morse_mode
                    #unilateral_tap
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

fn expand_macros(macros: &Option<MacrosConfig>) -> proc_macro2::TokenStream {
    let default = quote! { ::core::default::Default::default() };

    match macros {
        Some(macros) => {
            let macros_def = macros.macros.iter().map(|m| {
                let operations = m.operations.iter().map(|op| match op {
                    rmk_config::MacroOperation::Tap { keycode } => {
                        let key = get_key_with_alias(keycode.trim().to_owned());
                        quote! { ::rmk::keyboard_macros::MacroOperation::Tap(::rmk::keycode::KeyCode::#key).into_iter() }
                    }
                    rmk_config::MacroOperation::Down { keycode } => {
                        let key = get_key_with_alias(keycode.trim().to_owned());
                        quote! { ::rmk::keyboard_macros::MacroOperation::Press(::rmk::keycode::KeyCode::#key).into_iter() }
                    }
                    rmk_config::MacroOperation::Up { keycode } => {
                        let key = get_key_with_alias(keycode.trim().to_owned());
                        quote! { ::rmk::keyboard_macros::MacroOperation::Release(::rmk::keycode::KeyCode::#key).into_iter() }
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

fn expand_tap_dance(tap_dance: &Option<TapDancesConfig>) -> proc_macro2::TokenStream {
    let default = quote! { ::core::default::Default::default() };
    match tap_dance {
        Some(tap_dance) => {
            let tap_dances_def = tap_dance.tap_dances.iter().map(|td| {
                // Parse tapping term, default to 200ms if not specified
                let timeout = match &td.timeout {
                    Some(duration) => {
                        let millis = duration.0;
                        quote! { #millis }
                    }
                    None => quote! { 200 },
                };

                if td.tap_actions.is_some() || td.hold_actions.is_some() {
                    // Check first
                    if td.tap.is_some() || td.hold.is_some() || td.hold_after_tap.is_some() || td.double_tap.is_some() {
                        panic!("\n❌ keyboard.toml: `tap_actions` and `hold_actions` cannot be used together with `tap`, `hold`, `hold_after_tap`, or `double_tap`. Please check the documentation: https://rmk.rs/docs/features/configuration/behavior.html#tap-dance");
                    }
                    let tap_actions_def = match &td.tap_actions {
                        Some(tap_actions) => {
                            let actions = tap_actions.iter().map(|action| {
                                let parsed_action = parse_key(action.clone());
                                quote! { #parsed_action }
                            });
                            quote! { ::rmk::heapless::Vec::from_iter([#(#actions.to_action()),*]) }
                        }
                        None => quote! { ::rmk::heapless::Vec::new() },
                    };

                    let hold_actions_def = match &td.hold_actions {
                        Some(hold_actions) => {
                            let actions = hold_actions.iter().map(|action| {
                                let parsed_action = parse_key(action.clone());
                                quote! { #parsed_action }
                            });
                            quote! { ::rmk::heapless::Vec::from_iter([#(#actions.to_action()),*]) }
                        }
                        None => quote! { ::rmk::heapless::Vec::new() },
                    };

                    quote! {
                        ::rmk::tap_dance::TapDance::new_with_actions(
                            #tap_actions_def,
                            #hold_actions_def,
                            #timeout as u16
                        )
                    }
                } else {
                    let tap = parse_key(td.tap.clone().unwrap_or_else(|| "No".to_string()));
                    let hold = parse_key(td.hold.clone().unwrap_or_else(|| "No".to_string()));
                    let hold_after_tap = parse_key(td.hold_after_tap.clone().unwrap_or_else(|| "No".to_string()));
                    let double_tap = parse_key(td.double_tap.clone().unwrap_or_else(|| "No".to_string()));

                    quote! {
                        ::rmk::tap_dance::TapDance::new_from_vial(
                            #tap.to_action(),
                            #hold.to_action(),
                            #hold_after_tap.to_action(),
                            #double_tap.to_action(),
                            #timeout as u16
                        )
                    }
                }
            });

            quote! {
                ::rmk::config::TapDancesConfig {
                    tap_dances: ::rmk::heapless::Vec::from_iter([#(#tap_dances_def),*]),
                }
            }
        }
        None => default,
    }
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
                ::rmk::hid_state::HidModifiers::new_from(#left_ctrl, #left_shift, #left_alt, #left_gui, #right_ctrl, #right_shift, #right_alt, #right_gui),
                ::rmk::light::LedIndicator::new_from(#num_lock, #caps_lock, #scroll_lock, #compose, #kana),
                ::rmk::hid_state::HidMouseButtons::new_from(#button1, #button2, #button3, #button4, #button5, #button6, #button7, #button8))
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

fn expand_forks(forks: &Option<ForksConfig>) -> proc_macro2::TokenStream {
    let default = quote! { ::core::default::Default::default() };
    match forks {
        Some(forks) => {
            let forks_def = forks.forks.iter().map(|fork| {
                let trigger = parse_key(fork.trigger.to_owned());
                let negative_output = parse_key(fork.negative_output.to_owned());
                let positive_output = parse_key(fork.positive_output.to_owned());
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
    let behavior = keyboard_config.get_behavior_config().unwrap();
    let tri_layer = expand_tri_layer(&behavior.tri_layer);
    let morse = expand_morse(&behavior.tap_hold);
    let one_shot = expand_one_shot(&behavior.one_shot);
    let combos = expand_combos(&behavior.combo);
    let macros = expand_macros(&behavior.macros);
    let forks = expand_forks(&behavior.fork);
    let tap_dance = expand_tap_dance(&behavior.tap_dance);

    quote! {
        let behavior_config = ::rmk::config::BehaviorConfig {
            tri_layer: #tri_layer,
            morse: #morse,
            one_shot: #one_shot,
            combo: #combos,
            fork: #forks,
            tap_dance: #tap_dance,
            keyboard_macros: #macros,
            // keyboard_macros: ::rmk::config::macro_config::KeyboardMacrosConfig::default(),
            mouse_key: ::rmk::config::MouseKeyConfig::default(),
        };
    }
}
