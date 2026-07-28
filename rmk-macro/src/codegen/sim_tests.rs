//! Expand `run_tests!` scenario files into simulator `#[test]` fns.
//!
//! Each scenario TOML (parsed by `rmk_config::sim_tests`) becomes a
//! `mod <file_stem>` of tests targeting the `SimKeyboard` harness in rmk's
//! `tests/common/sim.rs`. Generated code is exactly what a hand-written test
//! would contain: a keymap array from the canonical `parse_key` pipeline, an
//! `expand_behavior_config` statement, builder calls, and timeline steps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::resolved::behavior::MorseProfile;
use rmk_config::sim_tests::{MouseSpec, SimTest, Step, parse_scenario_str, scenario_base_path};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token};

/// `run_tests!("tests/scenarios/foo.toml")` or
/// `run_tests!(keyboard = "tests/scenarios/boards/bar.toml", tests = "tests/scenarios/foo.toml")`.
pub(crate) struct RunTestsInput {
    keyboard: Option<String>,
    tests: String,
}

impl Parse for RunTestsInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(LitStr) {
            let tests: LitStr = input.parse()?;
            return Ok(Self {
                keyboard: None,
                tests: tests.value(),
            });
        }
        let mut keyboard = None;
        let mut tests = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "keyboard" => keyboard = Some(value.value()),
                "tests" => tests = Some(value.value()),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown argument `{other}` (expected `keyboard` or `tests`)"),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        let Some(tests) = tests else {
            return Err(input.error("missing `tests = \"...\"` argument"));
        };
        Ok(Self { keyboard, tests })
    }
}

pub(crate) fn expand_run_tests(input: RunTestsInput) -> TokenStream2 {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let tests_path = Path::new(&manifest_dir).join(&input.tests);
    let doc = read(&tests_path);
    let display = input.tests.clone();

    // A base keyboard comes from the macro invocation (manifest-relative) or
    // the scenario's own `keyboard = "path"` key (scenario-relative).
    let in_file_base = scenario_base_path(&doc).unwrap_or_else(|e| panic!("\n❌ {display}: {e}"));
    let base_path = match (&input.keyboard, &in_file_base) {
        (Some(_), Some(_)) => panic!(
            "\n❌ {display}: base keyboard given both as macro argument and as `keyboard = ...` in the scenario"
        ),
        (Some(arg), None) => Some(Path::new(&manifest_dir).join(arg)),
        (None, Some(rel)) => Some(
            tests_path
                .parent()
                .expect("scenario file has a parent")
                .join(rel),
        ),
        (None, None) => None,
    };
    let base = base_path.as_ref().map(|p| read(p));

    let scenario =
        parse_scenario_str(&doc, base.as_deref()).unwrap_or_else(|e| panic!("\n❌ {display}: {e}"));

    let stem = tests_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| panic!("\n❌ {display}: scenario file needs a UTF-8 file stem"));
    let mod_name = format_ident!("{}", stem.replace('-', "_"));

    let tests = scenario
        .tests
        .iter()
        .map(|test| expand_test(test, &display))
        .collect::<Vec<_>>();

    // Track the source files so cargo re-expands when they change.
    let tracked = [Some(&tests_path), base_path.as_ref()]
        .into_iter()
        .flatten()
        .map(|p| p.to_str().expect("scenario paths are UTF-8"))
        .map(|p| quote! { const _: &[u8] = include_bytes!(#p); });

    quote! {
        #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
        mod #mod_name {
            #(#tracked)*
            #(#tests)*
        }
    }
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("\n❌ cannot read {}: {e}", path.display()))
}

fn expand_test(test: &SimTest, file: &str) -> TokenStream2 {
    let ctx = format!("{file}, test '{}'", test.name);
    let keymap = test
        .config
        .keymap_headless()
        .unwrap_or_else(|e| panic!("\n❌ {ctx}: {e}"));
    let behavior = test
        .config
        .behavior()
        .unwrap_or_else(|e| panic!("\n❌ {ctx}: {e}"));
    let profiles: Option<HashMap<String, MorseProfile>> = behavior
        .morse
        .as_ref()
        .map(|m| m.profiles.clone())
        .filter(|p| !p.is_empty());

    let (rows, cols, layers) = (
        keymap.rows as usize,
        keymap.cols as usize,
        keymap.layers as usize,
    );
    let layer_tokens: Vec<_> = keymap
        .keymap
        .iter()
        .map(|layer| super::keymap::expand_layer(layer.clone(), &profiles))
        .collect();

    let mut builder_calls = TokenStream2::new();
    for row in 0..rows {
        for col in 0..cols {
            let hand = match keymap.key_info[row][col].hand {
                'l' | 'L' => quote! { ::rmk::config::Hand::Left },
                'r' | 'R' => quote! { ::rmk::config::Hand::Right },
                '*' => quote! { ::rmk::config::Hand::Bilateral },
                _ => continue,
            };
            builder_calls.extend(quote! { .hand(#row, #col, #hand) });
        }
    }
    for key in &test.keys {
        let (layer, row, col) = (key.layer as usize, key.row as usize, key.col as usize);
        if layer >= layers || row >= rows || col >= cols {
            panic!(
                "\n❌ {ctx}: key override ({layer}, {row}, {col}) is outside the {layers}x{rows}x{cols} keymap"
            );
        }
        let action = super::action_parser::parse_key(key.action.clone(), &profiles);
        builder_calls.extend(quote! { .key(#layer, #row, #col, #action) });
    }

    let behavior_stmt = super::behavior::expand_behavior_config(&behavior);
    let builder = quote! {
        crate::common::sim::SimKeyboard::builder::<#rows, #cols, #layers>([#(#layer_tokens),*])
            .behavior_config(behavior_config)
            #builder_calls
    };

    let mut phases: Vec<Vec<TokenStream2>> = vec![Vec::new()];
    for (index, step) in test.steps.iter().enumerate() {
        let step_ctx = format!("{ctx}, steps[{index}]");
        match step {
            Step::Restart => phases.push(Vec::new()),
            step => phases
                .last_mut()
                .expect("phases start non-empty")
                .push(expand_step(step, rows, cols, &step_ctx)),
        }
    }

    let phase_blocks = phases.iter().map(|steps| {
        let storage_call = test
            .storage
            .then(|| quote! { .storage_flash(flash.clone()) })
            .unwrap_or_default();
        quote! {
            {
                #behavior_stmt
                let mut keyboard = #builder #storage_call .build().await;
                keyboard #(#steps)* .run().await;
            }
        }
    });
    let flash_stmt = test.storage.then(|| {
        quote! { let flash = crate::common::sim::flash::InMemoryFlash::<4096, 256, 4>::new(); }
    });

    let cfg_attr = (!test.requires.is_empty()).then(|| {
        let features = test.requires.iter().map(|name| match name.as_str() {
            "ble" => "_ble".to_string(),
            "no_usb" => "_no_usb".to_string(),
            other => other.to_string(),
        });
        quote! { #[cfg(all(#(feature = #features),*))] }
    });

    let name = format_ident!("{}", test.name);
    quote! {
        #cfg_attr
        #[test]
        #[allow(unused_mut)]
        fn #name() {
            crate::common::test_block_on(async {
                #flash_stmt
                #(#phase_blocks)*
            });
        }
    }
}

fn expand_step(step: &Step, rows: usize, cols: usize, ctx: &str) -> TokenStream2 {
    let check_pos = |row: u8, col: u8| {
        if row as usize >= rows || col as usize >= cols {
            panic!("\n❌ {ctx}: position ({row}, {col}) is outside the {rows}x{cols} matrix");
        }
    };
    match step {
        Step::Press(row, col) => {
            check_pos(*row, *col);
            quote! { .press(#row, #col) }
        }
        Step::Release(row, col) => {
            check_pos(*row, *col);
            quote! { .release(#row, #col) }
        }
        Step::Tap(row, col, ms) => {
            check_pos(*row, *col);
            quote! { .tap(#row, #col, #ms) }
        }
        Step::Delay(ms) => quote! { .delay(#ms) },
        Step::NoReport(ms) => quote! { .expect_no_report(#ms) },
        Step::RotaryCw(_) | Step::RotaryCcw(_) => {
            panic!(
                "\n❌ {ctx}: rotary steps need an encoder map, which scenarios don't support yet"
            )
        }
        Step::Expect { mods, keys } => {
            let modifier = modifier_bits(mods, ctx);
            let keycodes = keys.iter().map(|key| {
                let ident = super::action_parser::get_key_with_alias(key.clone());
                quote! { ::rmk::types::keycode::HidKeyCode::#ident }
            });
            if keys.is_empty() {
                quote! { .expect_only_mods(#modifier) }
            } else if modifier == 0 {
                quote! { .expect_keys([#(#keycodes),*]) }
            } else {
                quote! { .expect_keys_with_mods(#modifier, [#(#keycodes),*]) }
            }
        }
        Step::ExpectEmpty => quote! { .expect_all_up() },
        Step::ExpectMouse(MouseSpec {
            buttons,
            x,
            y,
            wheel,
            pan,
        }) => quote! {
            .expect_report(::rmk::hid::Report::MouseReport(::usbd_hid::descriptor::MouseReport {
                buttons: #buttons,
                x: #x,
                y: #y,
                wheel: #wheel,
                pan: #pan,
            }))
        },
        Step::WaitStorage => quote! { .wait_storage() },
        Step::Restart => unreachable!("restart is split into phases before step expansion"),
    }
}

/// HID modifier byte from modifier names (boot-report bit order).
fn modifier_bits(mods: &[String], ctx: &str) -> u8 {
    let mut bits = 0u8;
    for name in mods {
        bits |= match name.as_str() {
            "LCtrl" => 0x01,
            "LShift" => 0x02,
            "LAlt" => 0x04,
            "LGui" => 0x08,
            "RCtrl" => 0x10,
            "RShift" => 0x20,
            "RAlt" => 0x40,
            "RGui" => 0x80,
            other => panic!(
                "\n❌ {ctx}: unknown modifier `{other}` (expected LCtrl/LShift/LAlt/LGui/RCtrl/RShift/RAlt/RGui)"
            ),
        };
    }
    bits
}
