//! Simulator scenario files: keyboard.toml sections plus `[[test]]` cases.
//!
//! A scenario document holds a keyboard definition (a `keyboard.toml` subset,
//! optionally deep-merged over a referenced base file) and an array of named
//! tests, each an ordered list of input/expectation steps. Parsing happens at
//! macro-expansion time — `rmk-macro`'s `run_tests!` turns each [`SimTest`]
//! into a generated `#[test]` against the simulator harness.

use std::collections::HashSet;

use serde::Deserialize;
use toml::{Table, Value};

use crate::KeyboardTomlConfig;

/// Cargo features a scenario may require. `ble`/`no_usb` map to rmk's internal
/// `_ble`/`_no_usb` feature names when the macro emits `#[cfg]`.
const KNOWN_FEATURES: &[&str] = &[
    "storage",
    "vial",
    "rynk",
    "host",
    "ble",
    "no_usb",
    "steno",
    "split",
    "passkey_entry",
    "async_matrix",
    "host_lock",
];

/// Sections a scenario may define. Everything else is either hardware-only
/// (meaningless in simulation) or compile-time (`[rmk]` capacities are baked
/// into the test binary by `rmk-types/build.rs`).
const SCENARIO_SECTIONS: &[&str] = &["layout", "keymap", "behavior", "aliases"];

pub struct Scenario {
    pub tests: Vec<SimTest>,
}

pub struct SimTest {
    pub name: String,
    /// Cargo features (public names) this test needs.
    pub requires: Vec<String>,
    /// Whether the test runs with an in-memory flash attached.
    pub storage: bool,
    pub steps: Vec<Step>,
    /// Keyboard config for this test: base file + scenario sections + per-test
    /// `[test.behavior]` delta, already merged.
    pub config: KeyboardTomlConfig,
}

pub enum Step {
    Press(u8, u8),
    Release(u8, u8),
    Tap(u8, u8, u64),
    Delay(u64),
    RotaryCw(u8),
    RotaryCcw(u8),
    /// Keyboard-report assertion: named modifiers plus an order-insensitive
    /// keycode set. Empty `mods` asserts modifier byte 0; empty `keys` asserts
    /// no keycodes.
    Expect {
        mods: Vec<String>,
        keys: Vec<String>,
    },
    /// Keyboard report with modifier 0 and no keycodes.
    ExpectEmpty,
    NoReport(u64),
    ExpectMouse(MouseSpec),
    WaitStorage,
    Restart,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct MouseSpec {
    pub buttons: u8,
    pub x: i8,
    pub y: i8,
    pub wheel: i8,
    pub pan: i8,
}

/// The `keyboard = "path"` base-file reference of a scenario document, if any.
///
/// The caller resolves and reads the path (relative to the scenario file) and
/// passes the content to [`parse_scenario_str`].
pub fn scenario_base_path(doc: &str) -> Result<Option<String>, String> {
    let table: Table = toml::from_str(doc).map_err(|e| format!("scenario TOML: {e}"))?;
    match table.get("keyboard") {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err("scenario TOML: `keyboard` must be a path string".to_string()),
    }
}

/// Parse a scenario document, merging its keyboard sections over an optional
/// base keyboard.toml.
pub fn parse_scenario_str(doc: &str, base: Option<&str>) -> Result<Scenario, String> {
    let mut doc_table: Table = toml::from_str(doc).map_err(|e| format!("scenario TOML: {e}"))?;

    doc_table.remove("keyboard"); // already consumed via `scenario_base_path`
    let file_requires = take_requires(&mut doc_table, "scenario")?;
    let tests_value = doc_table
        .remove("test")
        .ok_or_else(|| "scenario TOML: no [[test]] defined".to_string())?;

    for key in doc_table.keys() {
        if !SCENARIO_SECTIONS.contains(&key.as_str()) {
            return Err(format!(
                "scenario TOML: section [{key}] cannot take effect in sim tests (allowed: [layout], [keymap], [behavior], [aliases])"
            ));
        }
    }

    // From a base keyboard.toml, keep only the sections that matter in
    // simulation so a real keyboard's hardware sections need no stripping.
    let mut keyboard = match base {
        Some(base) => {
            let base_table: Table = toml::from_str(base).map_err(|e| format!("base keyboard TOML: {e}"))?;
            let mut kept = Table::new();
            for section in SCENARIO_SECTIONS {
                if let Some(v) = base_table.get(*section) {
                    kept.insert(section.to_string(), v.clone());
                }
            }
            kept
        }
        None => Table::new(),
    };
    deep_merge(&mut keyboard, doc_table);

    let Value::Array(raw_tests) = tests_value else {
        return Err("scenario TOML: `test` must be an array of tables ([[test]])".to_string());
    };
    if raw_tests.is_empty() {
        return Err("scenario TOML: no [[test]] defined".to_string());
    }

    let mut names = HashSet::new();
    let mut tests = Vec::with_capacity(raw_tests.len());
    for (index, raw) in raw_tests.into_iter().enumerate() {
        let test = parse_test(raw, index, &keyboard, &file_requires)?;
        if !names.insert(test.name.clone()) {
            return Err(format!("scenario TOML: duplicate test name '{}'", test.name));
        }
        tests.push(test);
    }
    Ok(Scenario { tests })
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

fn take_requires(table: &mut Table, ctx: &str) -> Result<Vec<String>, String> {
    let Some(value) = table.remove("requires") else {
        return Ok(Vec::new());
    };
    let list: Vec<String> = value
        .try_into()
        .map_err(|e| format!("{ctx}: `requires` must be an array of feature names: {e}"))?;
    for feature in &list {
        if !KNOWN_FEATURES.contains(&feature.as_str()) {
            return Err(format!(
                "{ctx}: unknown feature '{feature}' in `requires` (known: {})",
                KNOWN_FEATURES.join(", ")
            ));
        }
    }
    Ok(list)
}

fn parse_test(value: Value, index: usize, keyboard: &Table, file_requires: &[String]) -> Result<SimTest, String> {
    let Value::Table(mut table) = value else {
        return Err(format!("[[test]] #{index} must be a table"));
    };
    let name = match table.remove("name") {
        Some(Value::String(s)) => s,
        _ => return Err(format!("[[test]] #{index}: `name` (string) is required")),
    };
    let ctx = format!("test '{name}'");
    let mut chars = name.chars();
    let valid_ident = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid_ident {
        return Err(format!("{ctx}: name must be a valid Rust identifier"));
    }

    let mut requires = file_requires.to_vec();
    for feature in take_requires(&mut table, &ctx)? {
        if !requires.contains(&feature) {
            requires.push(feature);
        }
    }

    let storage = match table.remove("storage") {
        None => false,
        Some(Value::Boolean(b)) => b,
        Some(_) => return Err(format!("{ctx}: `storage` must be a boolean")),
    };
    if storage && !requires.iter().any(|f| f == "storage") {
        requires.push("storage".to_string());
    }

    let steps_value = table
        .remove("steps")
        .ok_or_else(|| format!("{ctx}: `steps` is required"))?;
    let behavior_delta = table.remove("behavior");
    if let Some(key) = table.keys().next() {
        return Err(format!("{ctx}: unknown key `{key}`"));
    }

    let Value::Array(raw_steps) = steps_value else {
        return Err(format!("{ctx}: `steps` must be an array"));
    };
    if raw_steps.is_empty() {
        return Err(format!("{ctx}: `steps` must not be empty"));
    }
    let steps = raw_steps
        .iter()
        .enumerate()
        .map(|(i, step)| parse_step(step).map_err(|e| format!("{ctx}, steps[{i}]: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    if !storage && steps.iter().any(|s| matches!(s, Step::WaitStorage | Step::Restart)) {
        return Err(format!(
            "{ctx}: \"wait_storage\"/\"restart\" steps need `storage = true`"
        ));
    }

    let mut config_table = keyboard.clone();
    if let Some(delta) = behavior_delta {
        let Value::Table(delta) = delta else {
            return Err(format!("{ctx}: [test.behavior] must be a table"));
        };
        let entry = config_table
            .entry("behavior".to_string())
            .or_insert(Value::Table(Table::new()));
        let Value::Table(base_behavior) = entry else {
            return Err(format!("{ctx}: [behavior] must be a table"));
        };
        deep_merge(base_behavior, delta);
    }
    let config: KeyboardTomlConfig = config_table
        .try_into()
        .map_err(|e| format!("{ctx}: keyboard config: {e}"))?;

    Ok(SimTest {
        name,
        requires,
        storage,
        steps,
        config,
    })
}

fn parse_step(value: &Value) -> Result<Step, String> {
    match value {
        Value::String(s) => match s.as_str() {
            "wait_storage" => Ok(Step::WaitStorage),
            "restart" => Ok(Step::Restart),
            other => Err(format!(
                "unknown step \"{other}\" (bare-string steps: \"wait_storage\", \"restart\")"
            )),
        },
        Value::Table(table) => {
            if table.len() != 1 {
                return Err("a step table must have exactly one key".to_string());
            }
            let (op, v) = table.iter().next().unwrap();
            match op.as_str() {
                "press" => pos(v).map(|(r, c)| Step::Press(r, c)),
                "release" => pos(v).map(|(r, c)| Step::Release(r, c)),
                "tap" => {
                    let (r, c, ms): (u8, u8, u64) = v
                        .clone()
                        .try_into()
                        .map_err(|e| format!("`tap` must be [row, col, hold_ms]: {e}"))?;
                    Ok(Step::Tap(r, c, ms))
                }
                "delay" => ms(v, "delay").map(Step::Delay),
                "no_report" => ms(v, "no_report").map(Step::NoReport),
                "rotary_cw" => id(v, "rotary_cw").map(Step::RotaryCw),
                "rotary_ccw" => id(v, "rotary_ccw").map(Step::RotaryCcw),
                "expect" => parse_expect(v),
                "expect_mouse" => {
                    let spec: MouseSpec = v
                        .clone()
                        .try_into()
                        .map_err(|e| format!("`expect_mouse` must be {{ buttons, x, y, wheel, pan }}: {e}"))?;
                    Ok(Step::ExpectMouse(spec))
                }
                other => Err(format!("unknown step op `{other}`")),
            }
        }
        _ => Err("a step must be a table or a keyword string".to_string()),
    }
}

fn parse_expect(value: &Value) -> Result<Step, String> {
    match value {
        Value::String(s) if s == "empty" => Ok(Step::ExpectEmpty),
        Value::String(s) => Err(format!("unknown expect \"{s}\" (did you mean \"empty\"?)")),
        Value::Array(_) => {
            let keys: Vec<String> = value
                .clone()
                .try_into()
                .map_err(|e| format!("`expect` keycodes must be strings: {e}"))?;
            if keys.is_empty() {
                return Err("`expect = []` is ambiguous; use `expect = \"empty\"`".to_string());
            }
            Ok(Step::Expect { mods: Vec::new(), keys })
        }
        Value::Table(_) => {
            #[derive(Deserialize, Default)]
            #[serde(deny_unknown_fields, default)]
            struct ExpectSpec {
                mods: Vec<String>,
                keys: Vec<String>,
            }
            let spec: ExpectSpec = value
                .clone()
                .try_into()
                .map_err(|e| format!("`expect` must be {{ mods = [..], keys = [..] }}: {e}"))?;
            if spec.mods.is_empty() && spec.keys.is_empty() {
                return Err("`expect` with neither mods nor keys is ambiguous; use `expect = \"empty\"`".to_string());
            }
            Ok(Step::Expect {
                mods: spec.mods,
                keys: spec.keys,
            })
        }
        _ => Err("`expect` must be a keycode array, { mods, keys }, or \"empty\"".to_string()),
    }
}

fn pos(value: &Value) -> Result<(u8, u8), String> {
    value
        .clone()
        .try_into()
        .map_err(|e| format!("expected [row, col]: {e}"))
}

fn ms(value: &Value, op: &str) -> Result<u64, String> {
    value
        .clone()
        .try_into()
        .map_err(|e| format!("`{op}` must be milliseconds (integer): {e}"))
}

fn id(value: &Value, op: &str) -> Result<u8, String> {
    value
        .clone()
        .try_into()
        .map_err(|e| format!("`{op}` must be an encoder id (integer): {e}"))
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

    #[test]
    fn minimal_scenario_parses() {
        let scenario = parse_scenario_str(MINIMAL, None).unwrap();
        assert_eq!(scenario.tests.len(), 1);
        assert_eq!(scenario.tests[0].steps.len(), 3);
        scenario.tests[0].config.keymap_headless().unwrap();
    }

    #[test]
    fn rmk_section_is_rejected() {
        let doc = format!("{MINIMAL}\n[rmk]\ncombo_max_num = 16\n");
        let err = parse_scenario_str(&doc, None).err().expect("expected error");
        assert!(err.contains("[rmk]"), "unexpected error: {err}");
    }

    #[test]
    fn unknown_step_op_is_rejected() {
        let doc = MINIMAL.replace("{ press = [0, 0] }", "{ pres = [0, 0] }");
        let err = parse_scenario_str(&doc, None).err().expect("expected error");
        assert!(
            err.contains("steps[0]") && err.contains("pres"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn storage_steps_need_storage_flag() {
        let doc = MINIMAL.replace("{ expect = \"empty\" }", "\"restart\"");
        let err = parse_scenario_str(&doc, None).err().expect("expected error");
        assert!(err.contains("storage = true"), "unexpected error: {err}");
    }

    #[test]
    fn duplicate_test_names_are_rejected() {
        let extra = "\n[[test]]\nname = \"t\"\nsteps = [{ delay = 1 }]\n";
        let err = parse_scenario_str(&format!("{MINIMAL}{extra}"), None)
            .err()
            .expect("expected error");
        assert!(err.contains("duplicate"), "unexpected error: {err}");
    }

    #[test]
    fn base_keyboard_sections_merge_under_scenario() {
        let base = "[layout]\nrows = 1\ncols = 2\nmap = \"(0,0) (0,1)\"\n\n[[keymap.layer]]\nkeys = \"A B\"\n\n[keyboard]\nname = \"real\"\n";
        let doc = r#"
[behavior.combo]
timeout = "40ms"

[[test]]
name = "t"
steps = [{ delay = 1 }]
"#;
        let scenario = parse_scenario_str(doc, Some(base)).unwrap();
        let keymap = scenario.tests[0].config.keymap_headless().unwrap();
        assert_eq!((keymap.rows, keymap.cols), (1, 2));
    }
}
