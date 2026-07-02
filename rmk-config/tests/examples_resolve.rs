//! Every bundled `use_config` example must parse and resolve. This guards the
//! `[layout].map` + `[[keymap.layer]]` authoring path: a stale legacy
//! `keymap = [[[…]]]` now trips `deny_unknown_fields`, and a mis-sized `map`
//! fails keymap resolution.

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
            config.keymap().unwrap_or_else(|e| panic!("keymap(): {e}"));
            config.layout().unwrap_or_else(|e| panic!("layout(): {e}"));
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

    assert!(failures.is_empty(), "examples failed to resolve:\n{}", failures.join("\n"));
}
