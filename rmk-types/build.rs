use std::path::Path;
use std::{env, fs};

use rmk_config::KeyboardTomlConfig;
use rmk_config::protocol_limits;
use rmk_config::resolved::BuildConstants;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KEYBOARD_TOML_PATH");
    println!("cargo:rerun-if-env-changed=VIAL_JSON_PATH");

    // Load keyboard.toml if it's present.
    //
    // Build-time constants only need [rmk] + [event]. Keep event defaults support
    // without requiring [keyboard.board]/[keyboard.chip].
    let config: KeyboardTomlConfig = if let Ok(toml_path) = std::env::var("KEYBOARD_TOML_PATH") {
        println!("cargo:rerun-if-changed={toml_path}");
        KeyboardTomlConfig::new_from_toml_path_with_event_defaults(&toml_path)
    } else {
        toml::from_str("").expect("Failed to parse empty keyboard config\n")
    };

    // Collect active feature flags.
    // The number of event subscribers bumps according to the enabled feature.
    let active_features = collect_active_features();
    let feature_refs: Vec<&str> = active_features.iter().map(|s| s.as_str()).collect();

    let bc = config
        .build_constants(&feature_refs)
        .unwrap_or_else(|err| panic!("Failed to resolve build constants: {err}"));
    let output = generate_constants(&bc);

    // Write to constants.rs file
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");
    fs::write(&dest_path, output).expect("Failed to write constants.rs file");
}

fn generate_constants(bc: &BuildConstants) -> String {
    let mut lines = Vec::new();

    // Direct constants
    lines.push(format!(
        "pub const MOUSE_KEY_INTERVAL: u16 = {};",
        bc.mouse_key_interval
    ));
    lines.push(format!(
        "pub const MOUSE_WHEEL_INTERVAL: u16 = {};",
        bc.mouse_wheel_interval
    ));
    lines.push(format!("pub const COMBO_MAX_NUM: usize = {};", bc.combo_max_num));
    lines.push(format!("pub const COMBO_MAX_LENGTH: usize = {};", bc.combo_max_length));
    lines.push(format!("pub const MACRO_SPACE_SIZE: usize = {};", bc.macro_space_size));
    lines.push(format!("pub const FORK_MAX_NUM: usize = {};", bc.fork_max_num));
    lines.push(format!("pub const DEBOUNCE_THRESHOLD: u16 = {};", bc.debounce_time));
    lines.push(format!(
        "pub const REPORT_CHANNEL_SIZE: usize = {};",
        bc.report_channel_size
    ));
    lines.push(format!(
        "pub const VIAL_CHANNEL_SIZE: usize = {};",
        bc.vial_channel_size
    ));
    lines.push(format!(
        "pub const FLASH_CHANNEL_SIZE: usize = {};",
        bc.flash_channel_size
    ));
    lines.push(format!(
        "pub const SPLIT_PERIPHERALS_NUM: usize = {};",
        bc.split_peripherals_num
    ));
    lines.push(format!("pub const NUM_BLE_PROFILE: usize = {};", bc.ble_profiles_num));
    lines.push(format!(
        "pub const SPLIT_CENTRAL_SLEEP_TIMEOUT_SECONDS: u32 = {};",
        bc.split_central_sleep_timeout_seconds
    ));
    lines.push(format!("pub const MORSE_MAX_NUM: usize = {};", bc.morse_max_num));
    lines.push(format!(
        "pub const MAX_PATTERNS_PER_KEY: usize = {};",
        bc.max_patterns_per_key
    ));

    // Protocol Vec capacity constants.
    //
    // There are two kinds of constants here:
    //
    // - **Internal capacity constants** (e.g., `COMBO_MAX_LENGTH`, `MAX_PATTERNS_PER_KEY`,
    //   `MACRO_SPACE_SIZE`) define how many combo keys, morse patterns, or macro bytes the
    //   firmware can store and process.
    //
    // - **Message Vec-capacity constants** (e.g., `COMBO_SIZE`, `MORSE_SIZE`,
    //   `MACRO_DATA_SIZE`, `BULK_SIZE`) define the maximum Vec capacity in protocol
    //   messages — how many elements can fit in a single request/response.
    //
    // On firmware, message capacities are typically set equal to their corresponding
    // internal constants (e.g., `COMBO_SIZE = COMBO_MAX_LENGTH`), because a single
    // protocol message needs to carry at most one full config.
    //
    // On the host side, message capacities use fixed upper bounds (e.g., 16, 32, 256) so
    // the host can deserialize responses from any firmware regardless of its config.
    //
    // `BULK_SIZE` is different: it controls multi-element bulk transfer (multiple
    // keys/combos/morses per message) and is only available behind the `bulk` feature.
    let is_host = env::var("CARGO_FEATURE_HOST").is_ok();
    let is_bulk = env::var("CARGO_FEATURE_BULK").is_ok();

    // Protocol ceilings — always emitted so rmk-types source code can reference them.
    lines.push(format!(
        "pub const MAX_COMBO_SIZE: usize = {};",
        protocol_limits::MAX_COMBO_SIZE
    ));
    lines.push(format!(
        "pub const MAX_MORSE_SIZE: usize = {};",
        protocol_limits::MAX_MORSE_SIZE
    ));
    lines.push(format!(
        "pub const MAX_MACRO_DATA_SIZE: usize = {};",
        protocol_limits::MAX_MACRO_DATA_SIZE
    ));
    lines.push(format!(
        "pub const MAX_BULK_SIZE: usize = {};",
        protocol_limits::MAX_BULK_SIZE
    ));

    if is_host {
        // Host: Vec capacities equal protocol ceilings for wire compatibility with any firmware.
        lines.push(format!(
            "pub const COMBO_SIZE: usize = {};",
            protocol_limits::MAX_COMBO_SIZE
        ));
        lines.push(format!(
            "pub const MORSE_SIZE: usize = {};",
            protocol_limits::MAX_MORSE_SIZE
        ));
        lines.push(format!(
            "pub const MACRO_DATA_SIZE: usize = {};",
            protocol_limits::MAX_MACRO_DATA_SIZE
        ));
        // Host always has bulk (host implies bulk feature)
        lines.push(format!(
            "pub const BULK_SIZE: usize = {};",
            protocol_limits::MAX_BULK_SIZE
        ));
    } else {
        // Firmware: per-item constants from keyboard.toml / defaults.
        lines.push(format!(
            "pub const COMBO_SIZE: usize = {};",
            bc.combo_max_length
        ));
        lines.push(format!(
            "pub const MORSE_SIZE: usize = {};",
            bc.max_patterns_per_key
        ));
        lines.push(format!(
            "pub const MACRO_DATA_SIZE: usize = {};",
            bc.protocol_macro_chunk_size
        ));
        // Compile-time check: firmware Vec sizes must not exceed protocol ceilings.
        // Only enforce when the rmk_protocol feature is active.
        if env::var("CARGO_FEATURE_RMK_PROTOCOL").is_ok() {
            lines.push("const _: () = assert!(COMBO_SIZE <= MAX_COMBO_SIZE, \"firmware COMBO_SIZE exceeds protocol ceiling MAX_COMBO_SIZE\");".to_string());
            lines.push("const _: () = assert!(MORSE_SIZE <= MAX_MORSE_SIZE, \"firmware MORSE_SIZE exceeds protocol ceiling MAX_MORSE_SIZE\");".to_string());
            lines.push("const _: () = assert!(MACRO_DATA_SIZE <= MAX_MACRO_DATA_SIZE, \"firmware MACRO_DATA_SIZE exceeds protocol ceiling MAX_MACRO_DATA_SIZE\");".to_string());
        }

        // Bulk constant only when bulk feature is active
        if is_bulk {
            lines.push(format!(
                "pub const BULK_SIZE: usize = {};",
                bc.protocol_max_bulk_size
            ));
            lines.push("const _: () = assert!(BULK_SIZE <= MAX_BULK_SIZE, \"firmware BULK_SIZE exceeds protocol ceiling MAX_BULK_SIZE\");".to_string());
        }
    }

    // Event channels
    for ev in &bc.events {
        let upper = ev.name.to_uppercase();
        lines.push(format!(
            "pub const {upper}_EVENT_CHANNEL_SIZE: usize = {};",
            ev.channel_size
        ));
        lines.push(format!("pub const {upper}_EVENT_PUB_SIZE: usize = {};", ev.pubs));
        lines.push(format!("pub const {upper}_EVENT_SUB_SIZE: usize = {};", ev.subs));
    }

    // Passkey (feature-gated)
    if env::var("CARGO_FEATURE_PASSKEY_ENTRY").is_ok() {
        if let Some(passkey) = &bc.passkey {
            lines.push(format!("pub const PASSKEY_ENTRY_ENABLED: bool = {};", passkey.enabled));
            lines.push(format!(
                "pub const PASSKEY_ENTRY_TIMEOUT_SECS: u32 = {};",
                passkey.timeout_secs
            ));
        } else {
            // No [ble] section but passkey_entry feature enabled: use defaults
            lines.push("pub const PASSKEY_ENTRY_ENABLED: bool = false;".to_string());
            lines.push(format!(
                "pub const PASSKEY_ENTRY_TIMEOUT_SECS: u32 = {};",
                rmk_config::DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS
            ));
        }
    }

    lines.join("\n")
}

/// Collect active Cargo feature flags from environment variables.
///
/// Cargo sets `CARGO_FEATURE_<NAME>` for each enabled feature (with the name
/// uppercased and `-` replaced by `_`). We reverse that to get lowercase names
/// matching the convention used in `subscriber_default.toml`.
fn collect_active_features() -> Vec<String> {
    env::vars()
        .filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(|f| f.to_lowercase()))
        .collect()
}
