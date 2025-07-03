#[path = "./build_common.rs"]
mod common;

use std::path::Path;
use std::process::Command;
use std::{env, fs};

use const_gen::*;
use rmk_config::{KeyboardTomlConfig, RmkConstantsConfig};

fn main() {
    // Set the compilation target configuration
    let mut cfgs = common::CfgSet::new();
    common::set_target_cfgs(&mut cfgs);

    // Ensure build.rs is re-run when files change
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KEYBOARD_TOML_PATH");
    println!("cargo:rerun-if-env-changed=VIAL_JSON_PATH");

    // Read keyboard.toml if it's present
    let user_config_str = if let Ok(toml_path) = std::env::var("KEYBOARD_TOML_PATH") {
        println!("cargo:rerun-if-changed={toml_path}");
        fs::read_to_string(&toml_path).expect("Failed to read user config file")
    } else {
        "".to_string()
    };

    // Parse user configuration
    let user_toml: KeyboardTomlConfig =
        toml::from_str(&user_config_str).expect("Failed to parse KEYBOARD_TOML_PATH file\n");

    // FIXME: calculate the number of split peripherals automatically
    // FIXME: calculate the number of controllers automatically

    let constants = get_constants_str(user_toml.rmk);

    // Write to constants.rs file
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");
    fs::write(&dest_path, constants).expect("Failed to write constants.rs file");
}

fn get_constants_str(constants: RmkConstantsConfig) -> String {
    // Compute build hash according to the latest git commit
    let build_hash = compute_build_hash();
    // Add other constants
    [
        const_declaration!(pub(crate) MOUSE_KEY_INTERVAL = constants.mouse_key_interval),
        const_declaration!(pub(crate) MOUSE_WHEEL_INTERVAL = constants.mouse_wheel_interval),
        const_declaration!(pub(crate) COMBO_MAX_NUM = constants.combo_max_num),
        const_declaration!(pub(crate) COMBO_MAX_LENGTH = constants.combo_max_length),
        const_declaration!(pub(crate) MACRO_SPACE_SIZE = constants.macro_space_size),
        const_declaration!(pub(crate) FORK_MAX_NUM = constants.fork_max_num),
        const_declaration!(pub(crate) DEBOUNCE_THRESHOLD = constants.debounce_time),
        const_declaration!(pub(crate) EVENT_CHANNEL_SIZE = constants.event_channel_size),
        const_declaration!(pub(crate) CONTROLLER_CHANNEL_SIZE = constants.controller_channel_size),
        const_declaration!(pub(crate) CONTROLLER_CHANNEL_PUBS = constants.controller_channel_pubs),
        const_declaration!(pub(crate) CONTROLLER_CHANNEL_SUBS = constants.controller_channel_subs),
        const_declaration!(pub(crate) REPORT_CHANNEL_SIZE = constants.report_channel_size),
        const_declaration!(pub(crate) VIAL_CHANNEL_SIZE = constants.vial_channel_size),
        const_declaration!(pub(crate) FLASH_CHANNEL_SIZE = constants.flash_channel_size),
        const_declaration!(pub(crate) SPLIT_PERIPHERALS_NUM = constants.split_peripherals_num),
        const_declaration!(pub(crate) SPLIT_MESSAGE_CHANNEL_SIZE = constants.split_message_channel_size),
        const_declaration!(pub(crate) NUM_BLE_PROFILE = constants.ble_profiles_num),
        const_declaration!(pub(crate) SPLIT_CENTRAL_SLEEP_TIMEOUT_MINUTES = constants.split_central_sleep_timeout_minutes),
        const_declaration!(pub(crate) TAP_DANCE_MAX_NUM = constants.tap_dance_max_num),
        format!("pub(crate) const BUILD_HASH: u32 = {build_hash:#010x};\n"),
    ]
    .map(|s| "#[allow(clippy::redundant_static_lifetimes)]\n".to_owned() + s.as_str())
    .join("\n")
}

fn compute_build_hash() -> u32 {
    // Get the short hash of the latest Git commit. Use "unknown" if it fails
    let commit_id = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Get and format current local time
    let now = chrono::Local::now();

    // Combine data and compute CRC32
    let combined = format!("{commit_id}_{now}");
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(combined.as_bytes());
    hasher.finalize()
}
