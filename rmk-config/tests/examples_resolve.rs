//! Every bundled `use_config` example must parse and resolve through the same
//! views `#[rmk_keyboard]` consumes. This guards the whole authoring surface:
//! unknown keys anywhere trip `deny_unknown_fields`, a stale legacy
//! `keymap = [[[…]]]` is rejected, and a mis-sized `map` fails keymap
//! resolution.

use std::path::Path;

use rmk_config::KeyboardTomlConfig;

#[test]
fn all_use_config_examples_resolve() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/use_config");

    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .expect("read examples/use_config")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.join("keyboard.toml").exists())
        .collect();
    dirs.sort();

    // `new_from_toml_path` panics on bad config, so collect per-example results
    // to report every failure at once instead of aborting on the first.
    std::panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let toml = dir.join("keyboard.toml");
        let outcome = std::panic::catch_unwind(|| {
            let config = KeyboardTomlConfig::new_from_toml_path(&toml);
            config.identity().unwrap_or_else(|e| panic!("identity(): {e}"));
            config.hardware().unwrap_or_else(|e| panic!("hardware(): {e}"));
            config.behavior().unwrap_or_else(|e| panic!("behavior(): {e}"));
            config.keymap().unwrap_or_else(|e| panic!("keymap(): {e}"));
            config.layout().unwrap_or_else(|e| panic!("layout(): {e}"));
            config.host();
        });
        if let Err(payload) = outcome {
            let msg = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("<panic>");
            failures.push(format!("{name}: {msg}"));
        }
    }
    let _ = std::panic::take_hook();

    assert!(
        failures.is_empty(),
        "examples failed to resolve:\n{}",
        failures.join("\n")
    );
}

/// Unknown keys in the sections users edit most must be rejected, not
/// silently dropped (pre-fix they surfaced as a misleading "X is required"
/// error that never named the typo).
#[test]
fn unknown_keys_are_rejected() {
    let cases = [
        ("top-level section typo", "[matirx]\nrow_pins = []\n", "matirx"),
        ("[matrix] field typo", "[matrix]\nrow_pin = [\"P0_01\"]\n", "row_pin"),
        (
            "[keyboard] field typo",
            "[keyboard]\nname = \"x\"\nvendor_di = 1\n",
            "vendor_di",
        ),
    ];

    std::panic::set_hook(Box::new(|_| {}));
    for (case, toml, typo) in cases {
        let path = std::env::temp_dir().join(format!("rmk-deny-{}-{typo}.toml", std::process::id()));
        std::fs::write(&path, toml).unwrap();
        let result = std::panic::catch_unwind(|| KeyboardTomlConfig::new_from_toml_path_with_event_defaults(&path));
        std::fs::remove_file(&path).ok();

        let payload = result.err().unwrap_or_else(|| panic!("{case}: accepted silently"));
        let msg = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<panic>");
        assert!(
            msg.contains("unknown field") && msg.contains(typo),
            "{case}: error should name `{typo}`, got: {msg}"
        );
    }
    let _ = std::panic::take_hook();
}
