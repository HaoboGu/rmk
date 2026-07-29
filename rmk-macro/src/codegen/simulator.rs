//! `run_tests!`: expand a simulator scenario file into `#[test]` fns.
//!
//! A scenario TOML holds a keyboard definition (a `keyboard.toml`, optionally
//! deep-merged over a referenced base board) and an array of named `[[test]]`
//! cases, each an ordered list of input/expectation steps. The keyboard half
//! deserializes into the ordinary [`KeyboardTomlConfig`], so a real keyboard's
//! `keyboard.toml` works as a board as-is — simulation reads only its keymap
//! and behavior, and never resolves its hardware.
//!
//! Each file becomes a `mod <file_stem>` of tests driving the `SimKeyboard`
//! harness in rmk's `tests/common/simulator.rs`, expanding to what a
//! hand-written test would contain.

use std::fs;
use std::path::Path;

use crate::codegen::action_parser::get_key_with_alias;
use crate::codegen::behavior::expand_behavior_config;
use crate::codegen::keymap::{expand_encoder_layer, expand_layer};
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use rmk_config::KeyboardTomlConfig;
use rmk_config::resolved::{Behavior, Keymap};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use syn::LitStr;
use toml::{Table, Value};

/// `run_tests!("tests/scenarios/foo.toml")`, with the path relative to
/// `Cargo.toml`.
pub(crate) fn expand_run_tests(scenario: LitStr) -> TokenStream2 {
    let path = scenario.value();
    expand_file(&path).unwrap_or_else(|e| panic!("\n❌ {path}: {e}"))
}

/// Read the scenario and its base board, then expand the whole file.
fn expand_file(relative: &str) -> Result<TokenStream2, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let path = Path::new(&manifest_dir).join(relative);
    let doc =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut doc: Table = toml::from_str(&doc).map_err(|e| format!("scenario TOML: {e}"))?;

    // `keyboard = "..."` names the base board, relative to the scenario file.
    let base_path = match doc.remove("keyboard") {
        None => None,
        Some(Value::String(rel)) => Some(path.parent().expect("scenario has a parent").join(rel)),
        Some(_) => return Err("`keyboard` must be a path string".to_string()),
    };
    let read =
        |p: &Path| fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()));
    let base = base_path.as_deref().map(read).transpose()?;

    let tests = expand_scenario(doc, base.as_deref())?;
    let stem = path.file_stem().and_then(|s| s.to_str());
    let stem = stem.expect("scenario paths are UTF-8 and name a file");
    let mod_name = format_ident!("{}", stem.replace('-', "_"));

    // Track the source files so cargo re-expands when they change.
    let paths = [Some(&path), base_path.as_ref()].into_iter().flatten();
    let paths = paths.map(|p| p.to_str().expect("scenario paths are UTF-8"));
    let tracked = paths.map(|p| quote! { const _: &[u8] = include_bytes!(#p); });

    Ok(quote! {
        mod #mod_name {
            #(#tracked)*
            #(#tests)*
        }
    })
}

/// Expand every `[[test]]` of a scenario whose `keyboard = "path"` reference is
/// already resolved into `base`.
fn expand_scenario(mut doc: Table, base: Option<&str>) -> Result<Vec<TokenStream2>, String> {
    let raw_tests = match doc.remove("test") {
        Some(Value::Array(tests)) if !tests.is_empty() => tests,
        _ => return Err("scenario TOML: no [[test]] defined".to_string()),
    };
    // `[rmk]` capacities are compile-time constants baked into the test binary by
    // `rmk-types/build.rs`, so a scenario's would silently do nothing.
    if doc.contains_key("rmk") {
        return Err("scenario TOML: [rmk] capacities are compile-time constants and cannot take effect in sim tests".to_string());
    }
    let file_features: Vec<String> = match doc.remove("features") {
        None => Vec::new(),
        Some(value) => arg(&value, "features", "an array of cargo feature names")?,
    };

    let mut keyboard = match base {
        Some(base) => toml::from_str(base).map_err(|e| format!("base keyboard TOML: {e}"))?,
        None => Table::new(),
    };
    deep_merge(&mut keyboard, doc);

    // Duplicate test names need no check here: they collide as fn names in the
    // generated mod, which rustc reports.
    let raw_tests = raw_tests.into_iter().enumerate();
    raw_tests
        .map(|(i, raw)| expand_test(raw, i, &keyboard, &file_features))
        .collect()
}

/// `#[cfg(all(feature = "..", ..))]` gating a test on the cargo features its
/// steps need, so a scenario still compiles in feature sets that lack them.
fn expand_features(features: &[String]) -> TokenStream2 {
    if features.is_empty() {
        return TokenStream2::new();
    }
    quote! { #[cfg(all(#(feature = #features),*))] }
}

/// Merge `over` into `base`: tables merge recursively, everything else
/// (arrays included) replaces wholesale.
fn deep_merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Table(b)), Value::Table(o)) => deep_merge(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

/// Expand one `[[test]]` into a `#[test]` fn. `file_features` are the scenario's
/// own `features`, which every test in it inherits.
fn expand_test(
    value: Value,
    index: usize,
    keyboard: &Table,
    file_features: &[String],
) -> Result<TokenStream2, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Test {
        name: String,
        steps: Vec<Value>,
        behavior: Option<Value>,
        #[serde(default)]
        features: Vec<String>,
    }
    let test: Result<Test, _> = value.try_into();
    let test = test.map_err(|e| format!("[[test]] #{index}: {e}"))?;
    let ctx = format!("test '{}'", test.name);
    if test.steps.is_empty() {
        return Err(format!("{ctx}: `steps` must not be empty"));
    }

    let mut config_table = keyboard.clone();
    let delta = test.behavior.map(|d| ("behavior".to_string(), d));
    deep_merge(&mut config_table, delta.into_iter().collect());
    let config: Result<KeyboardTomlConfig, _> = config_table.try_into();
    let config = config.map_err(|e| format!("{ctx}: keyboard config: {e}"))?;
    let keymap = config.keymap().map_err(|e| format!("{ctx}: {e}"))?;
    let behavior = config.behavior().map_err(|e| format!("{ctx}: {e}"))?;

    let mut steps = Vec::new();
    for (i, raw) in test.steps.iter().enumerate() {
        steps.push(step(&keymap, raw).map_err(|e| format!("{ctx}, steps[{i}]: {e}"))?);
    }

    let behavior_stmt = expand_behavior_config(&behavior);
    let builder = expand_builder(&keymap, &behavior);
    let fn_name = format_ident!("{}", test.name);
    let features = expand_features(&[file_features, &test.features].concat());

    Ok(quote! {
        #features
        #[test]
        fn #fn_name() {
            crate::common::test_block_on(async {
                #behavior_stmt
                let mut keyboard = #builder .build().await;
                keyboard #(#steps)* .run().await;
            });
        }
    })
}

/// The `SimKeyboard::builder(..)` chain for one test: keymap layers, encoder
/// layers, and per-key handedness. `.build()` is left to the caller.
fn expand_builder(keymap: &Keymap, behavior: &Behavior) -> TokenStream2 {
    let profiles = behavior.morse.as_ref().map(|m| m.profiles.clone());
    let rows = keymap.rows as usize;
    let cols = keymap.cols as usize;
    let layers = keymap.layers as usize;
    let layer_tokens = keymap.keymap.iter();
    let layer_tokens = layer_tokens.map(|l| expand_layer(l.clone(), &profiles));

    let num_encoder = keymap.num_encoder;
    let encoder_call = (num_encoder > 0).then(|| {
        // `expand_encoder_layer` pads a layer that lists no `encoders` with `No`.
        let mut encoder_map = keymap.encoder_map.clone();
        encoder_map.resize(layers, Vec::new());
        let encoder_layers = encoder_map
            .into_iter()
            .map(|e| expand_encoder_layer(e, num_encoder, &profiles));
        quote! { .encoders([#(#encoder_layers),*]) }
    });

    let hand_calls = (0..rows).flat_map(|row| (0..cols).map(move |col| (row, col)));
    let hand_calls = hand_calls.filter_map(|(row, col)| {
        let hand = match keymap.key_info[row][col].hand {
            'l' | 'L' => quote! { ::rmk::config::Hand::Left },
            'r' | 'R' => quote! { ::rmk::config::Hand::Right },
            '*' => quote! { ::rmk::config::Hand::Bilateral },
            _ => return None,
        };
        Some(quote! { .hand(#row, #col, #hand) })
    });
    quote! {
        crate::common::simulator::SimKeyboard::builder::<#rows, #cols, #layers>([#(#layer_tokens),*])
            .behavior_config(behavior_config)
            #encoder_call
            #(#hand_calls)*
    }
}

/// One step as the harness call it appends to the keyboard's timeline. Positions
/// and encoder ids are bounds-checked here because the runtime keymap indexes a
/// flat array — an out-of-range one silently reads a neighbouring key or layer.
fn step(keymap: &Keymap, value: &Value) -> Result<TokenStream2, String> {
    let one_op = value.as_table().filter(|t| t.len() == 1);
    let Some((op, v)) = one_op.and_then(|t| t.iter().next()) else {
        return Err("a step must be a table with exactly one key".to_string());
    };
    Ok(match op.as_str() {
        // `press`/`release` and `rotary_*` name their harness method directly.
        "press" | "release" => {
            let (row, col): (u8, u8) = arg(v, op, "[row, col]")?;
            check_key(keymap, row, col)?;
            let method = format_ident!("{op}");
            quote! { .#method(#row, #col) }
        }
        "tap" => {
            let (row, col, ms): (u8, u8, u64) = arg(v, op, "[row, col, hold_ms]")?;
            check_key(keymap, row, col)?;
            quote! { .tap(#row, #col, #ms) }
        }
        "delay" => {
            let ms: u64 = arg(v, op, "milliseconds (integer)")?;
            quote! { .delay(#ms) }
        }
        "no_report" => {
            let ms: u64 = arg(v, op, "milliseconds (integer)")?;
            quote! { .expect_no_report(#ms) }
        }
        "rotary_cw" | "rotary_ccw" => {
            let id: u8 = arg(v, op, "an encoder id (integer)")?;
            let encoders = keymap.num_encoder;
            if id as usize >= encoders {
                return Err(format!("encoder {id} is outside the {encoders} declared"));
            }
            let method = format_ident!("{op}");
            quote! { .#method(#id) }
        }
        "expect" => expect(v)?,
        "expect_mouse" => {
            #[derive(Deserialize, Default)]
            #[serde(deny_unknown_fields, default)]
            struct Mouse {
                buttons: u8,
                x: i8,
                y: i8,
                wheel: i8,
                pan: i8,
            }
            let m: Mouse = arg(v, op, "{ buttons, x, y, wheel, pan }")?;
            let (buttons, x, y, wheel, pan) = (m.buttons, m.x, m.y, m.wheel, m.pan);
            quote! {
                .expect_report(::rmk::hid::Report::MouseReport(::usbd_hid::descriptor::MouseReport {
                    buttons: #buttons,
                    x: #x,
                    y: #y,
                    wheel: #wheel,
                    pan: #pan,
                }))
            }
        }
        // The consumer/system/steno reports carry one value each, so they name it
        // in the same keycode vocabulary the keymap uses; "empty" is the release.
        "expect_consumer" => {
            let usage_id = match usage_key(v, op)? {
                None => quote! { 0 },
                Some(key) => quote! {
                    u16::from(
                        ::rmk::types::keycode::HidKeyCode::#key
                            .process_as_consumer()
                            .expect("`expect_consumer` needs a consumer key"),
                    )
                },
            };
            quote! {
                .expect_report(::rmk::hid::Report::MediaKeyboardReport(
                    ::usbd_hid::descriptor::MediaKeyboardReport { usage_id: #usage_id },
                ))
            }
        }
        "expect_system" => {
            let usage_id = match usage_key(v, op)? {
                None => quote! { 0 },
                Some(key) => quote! {
                    ::rmk::types::keycode::HidKeyCode::#key
                        .process_as_system_control()
                        .expect("`expect_system` needs a system control key") as u8
                },
            };
            quote! {
                .expect_report(::rmk::hid::Report::SystemControlReport(
                    ::usbd_hid::descriptor::SystemControlReport { usage_id: #usage_id },
                ))
            }
        }
        "expect_steno" => {
            let keys: Vec<String> = match v {
                Value::String(s) if s == "empty" => Vec::new(),
                _ => arg(v, op, "steno key names or \"empty\"")?,
            };
            // A `StenoKey` is a bit index into the 64-bit chord bitmap. The array
            // is annotated so an empty chord ("empty") still infers.
            let len = keys.len();
            let keys = keys.iter().map(|k| format_ident!("{}", k.to_uppercase()));
            quote! {
                .expect_report(::rmk::hid::Report::StenoReport({
                    let chord: [::rmk::types::steno::StenoKey; #len] =
                        [#(::rmk::types::steno::StenoKey::#keys),*];
                    let mut keys = [0u8; 8];
                    for key in chord {
                        keys[(key.0 / 8) as usize] |= 0x80 >> (key.0 % 8);
                    }
                    ::rmk::hid::StenoReport { keys }
                }))
            }
        }
        // Passkey entry brackets a stretch of the timeline: while it is open the
        // keys feed the passkey instead of the host, so nothing is reported.
        "passkey" => match arg::<String>(v, op, "\"begin\" or \"end\"")?.as_str() {
            "begin" => quote! { .begin_passkey_entry() },
            "end" => quote! { .end_passkey_entry() },
            other => return Err(format!("unknown `passkey` value \"{other}\"")),
        },
        "expect_passkey" => match v {
            Value::String(s) if s == "cancelled" => quote! { .expect_passkey_response(None) },
            Value::Integer(_) => {
                let passkey: u32 = arg(v, op, "a passkey or \"cancelled\"")?;
                quote! { .expect_passkey_response(Some(#passkey)) }
            }
            _ => return Err(format!("`{op}` must be a passkey or \"cancelled\"")),
        },
        other => return Err(format!("unknown step op `{other}`")),
    })
}

/// The key named by a single-usage report expectation, or `None` for `"empty"`
/// (the all-released report both pages send on key up).
fn usage_key(value: &Value, op: &str) -> Result<Option<Ident>, String> {
    match value {
        Value::String(s) if s == "empty" => Ok(None),
        Value::String(s) => Ok(Some(get_key_with_alias(s.clone()))),
        _ => Err(format!("`{op}` must be a key name or \"empty\"")),
    }
}

fn check_key(keymap: &Keymap, row: u8, col: u8) -> Result<(), String> {
    let (rows, cols) = (keymap.rows, keymap.cols);
    if row >= rows || col >= cols {
        return Err(format!(
            "position ({row}, {col}) is outside the {rows}x{cols} matrix"
        ));
    }
    Ok(())
}

/// Deserialize a step's argument, naming the shape it should have on failure.
fn arg<T: DeserializeOwned>(value: &Value, op: &str, shape: &str) -> Result<T, String> {
    let parsed: Result<T, _> = value.clone().try_into();
    parsed.map_err(|e| format!("`{op}` must be {shape}: {e}"))
}

/// A keyboard-report assertion: `"empty"`, a keycode array, or named modifiers
/// plus an order-insensitive keycode set.
fn expect(value: &Value) -> Result<TokenStream2, String> {
    let (mods, keys) = match value {
        Value::String(s) if s == "empty" => return Ok(quote! { .expect_all_up() }),
        Value::Array(_) => (0u8, arg::<Vec<String>>(value, "expect", "keycode strings")?),
        Value::Table(_) => {
            #[derive(Deserialize, Default)]
            #[serde(deny_unknown_fields, default)]
            struct Expect {
                mods: Vec<String>,
                keys: Vec<String>,
            }
            let spec: Expect = arg(value, "expect", "{ mods = [..], keys = [..] }")?;
            (modifier_bits(&spec.mods)?, spec.keys)
        }
        _ => {
            return Err(
                "`expect` must be a keycode array, { mods, keys }, or \"empty\"".to_string(),
            );
        }
    };
    // `expect_keys`/`expect_only_mods` are just this call with one side empty.
    let keycodes = keys.iter().map(|key| {
        let ident = get_key_with_alias(key.clone());
        quote! { ::rmk::types::keycode::HidKeyCode::#ident }
    });
    Ok(quote! { .expect_keys_with_mods(#mods, [#(#keycodes),*]) })
}

/// HID modifier byte from modifier names. `NAMES` is in boot-report bit order,
/// so a name's index is its bit.
fn modifier_bits(mods: &[String]) -> Result<u8, String> {
    const NAMES: [&str; 8] = [
        "LCtrl", "LShift", "LAlt", "LGui", "RCtrl", "RShift", "RAlt", "RGui",
    ];
    mods.iter().try_fold(0u8, |bits, name| {
        let bit = NAMES
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| format!("unknown modifier `{name}` (expected {})", NAMES.join("/")))?;
        Ok(bits | 1 << bit)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
[layout]
rows = 1
cols = 1
map = "(0,0)"

[[keymap.layer]]
keys = "A"

[[test]]
name = "t"
steps = [{ press = [0, 0] }, { release = [0, 0] }, { expect = "empty" }]
"#;

    /// Expand a scenario the way `expand_file` does, minus the file reads.
    fn expand(doc: &str, base: Option<&str>) -> Result<String, String> {
        let doc: Table = toml::from_str(doc).expect("fixture is valid TOML");
        let tests = expand_scenario(doc, base)?;
        Ok(quote! { #(#tests)* }.to_string().replace(' ', ""))
    }

    #[test]
    fn minimal_scenario_expands() {
        let tests = expand(MINIMAL, None).unwrap();
        assert!(tests.contains("fnt()"), "unexpected expansion: {tests}");
        assert!(
            tests.contains(".press(0u8,0u8).release(0u8,0u8).expect_all_up()"),
            "unexpected expansion: {tests}"
        );
    }

    #[test]
    fn rmk_section_is_rejected() {
        let doc = format!("{MINIMAL}\n[rmk]\ncombo_max_num = 16\n");
        let err = expand(&doc, None).expect_err("expected error");
        assert!(err.contains("[rmk]"), "unexpected error: {err}");
    }

    #[test]
    fn unknown_step_op_is_rejected() {
        let doc = MINIMAL.replace("{ press = [0, 0] }", "{ pres = [0, 0] }");
        let err = expand(&doc, None).expect_err("expected error");
        assert!(
            err.contains("steps[0]") && err.contains("pres"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn out_of_range_position_is_rejected() {
        let doc = MINIMAL.replace("{ press = [0, 0] }", "{ press = [0, 3] }");
        let err = expand(&doc, None).expect_err("expected error");
        assert!(err.contains("1x1 matrix"), "unexpected error: {err}");
    }

    /// A real `keyboard.toml` works as a board: its hardware sections resolve
    /// harmlessly and `[input_device]` still declares the encoders.
    #[test]
    fn base_keyboard_toml_resolves_with_hardware_sections() {
        let base = "[keyboard]\nname = \"real\"\nvendor_id = 0x4b4d\nproduct_id = 0x4b31\nchip = \"nrf52840\"\n\
             [matrix]\nrow_pins = [\"r0\"]\ncol_pins = [\"c0\"]\n\
             [ble]\nenabled = true\n\
             [layout]\nrows = 1\ncols = 1\nmap = \"(0,0)\"\n\
             [[keymap.layer]]\nkeys = \"A\"\nencoders = [[\"Up\", \"Down\"]]\n\
             [[input_device.encoder]]\npin_a = \"a0\"\npin_b = \"b0\"\n";
        let doc = "[[test]]\nname = \"t\"\nsteps = [{ rotary_cw = 0 }]\n";
        let tests = expand(doc, Some(base)).unwrap();
        assert!(
            tests.contains(".encoders(") && tests.contains(".rotary_cw(0u8)"),
            "unexpected expansion: {tests}"
        );
    }

    /// Encoders spread across split halves still total up board-wide.
    #[test]
    fn split_board_encoders_are_summed() {
        let base = "[layout]\nrows = 1\ncols = 2\nmap = \"(0,0) (0,1)\"\n\
             [[keymap.layer]]\nkeys = \"A B\"\nencoders = [[\"Up\", \"Down\"], [\"Left\", \"Right\"]]\n\
             [split]\nconnection = \"ble\"\n\
             [split.central]\nrows = 1\ncols = 1\nrow_offset = 0\ncol_offset = 0\n\
             matrix = { row_pins = [\"r0\"], col_pins = [\"c0\"] }\n\
             [[split.central.input_device.encoder]]\npin_a = \"a0\"\npin_b = \"b0\"\n\
             [[split.peripheral]]\nrows = 1\ncols = 1\nrow_offset = 0\ncol_offset = 1\n\
             matrix = { row_pins = [\"r1\"], col_pins = [\"c1\"] }\n\
             [[split.peripheral.input_device.encoder]]\npin_a = \"a1\"\npin_b = \"b1\"\n";
        // A two-encoder layer map only resolves if both halves are counted.
        let doc = "[[test]]\nname = \"t\"\nsteps = [{ rotary_ccw = 1 }]\n";
        let tests = expand(doc, Some(base)).unwrap();
        assert!(
            tests.contains(".rotary_ccw(1u8)"),
            "unexpected expansion: {tests}"
        );
    }

    /// A scenario may resize a real board: `[matrix]` pin counts are hardware,
    /// and simulation never checks them against `[layout]`.
    #[test]
    fn scenario_may_resize_a_real_board() {
        let base = "[matrix]\nrow_pins = [\"r0\", \"r1\"]\ncol_pins = [\"c0\", \"c1\", \"c2\"]\n\
             [layout]\nrows = 2\ncols = 3\nmap = \"(0,0) (0,1) (0,2)\\n(1,0) (1,1) (1,2)\"\n\
             [[keymap.layer]]\nkeys = \"A B C D E F\"\n";
        let doc = "[layout]\nrows = 1\ncols = 1\nmap = \"(0,0)\"\n\
             [[keymap.layer]]\nkeys = \"A\"\n\
             [[test]]\nname = \"t\"\nsteps = [{ press = [0, 0] }]\n";
        let tests = expand(doc, Some(base)).unwrap();
        assert!(
            tests.contains("builder::<1usize,1usize,1usize>"),
            "unexpected expansion: {tests}"
        );
    }
}
