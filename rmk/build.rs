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
    // println!("cargo:rerun-if-changed=.git/HEAD");
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
    let mut user_toml: KeyboardTomlConfig =
        toml::from_str(&user_config_str).expect("Failed to parse KEYBOARD_TOML_PATH file\n");

    // FIXME: calculate the number of controllers automatically
    user_toml.auto_calculate_parameters();

    // Fix the default split_peripherals_num when `split` feature is enabled
    if env::var("CARGO_FEATURE_SPLIT").is_ok() && user_toml.rmk.split_peripherals_num < 1 {
        user_toml.rmk.split_peripherals_num = 1;
    }

    let constants = get_constants_str(user_toml.rmk, user_toml.event.with_defaults());

    // Write to constants.rs file
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");
    fs::write(&dest_path, constants).expect("Failed to write constants.rs file");
}

fn get_constants_str(constants: RmkConstantsConfig, events: rmk_config::EventConfig) -> String {
    // Compute build hash according to the latest git commit
    let build_hash = compute_build_hash();
    // Add other constants
    let mut constant_strs = vec![
        const_declaration!(pub(crate) MOUSE_KEY_INTERVAL = constants.mouse_key_interval),
        const_declaration!(pub(crate) MOUSE_WHEEL_INTERVAL = constants.mouse_wheel_interval),
        const_declaration!(pub(crate) COMBO_MAX_NUM = constants.combo_max_num),
        const_declaration!(pub(crate) COMBO_MAX_LENGTH = constants.combo_max_length),
        const_declaration!(pub(crate) MACRO_SPACE_SIZE = constants.macro_space_size),
        const_declaration!(pub(crate) FORK_MAX_NUM = constants.fork_max_num),
        const_declaration!(pub(crate) DEBOUNCE_THRESHOLD = constants.debounce_time),
        const_declaration!(pub(crate) REPORT_CHANNEL_SIZE = constants.report_channel_size),
        const_declaration!(pub(crate) VIAL_CHANNEL_SIZE = constants.vial_channel_size),
        const_declaration!(pub(crate) FLASH_CHANNEL_SIZE = constants.flash_channel_size),
        const_declaration!(pub(crate) SPLIT_PERIPHERALS_NUM = constants.split_peripherals_num),
        const_declaration!(pub(crate) NUM_BLE_PROFILE = constants.ble_profiles_num),
        const_declaration!(pub(crate) SPLIT_CENTRAL_SLEEP_TIMEOUT_SECONDS = constants.split_central_sleep_timeout_seconds),
        const_declaration!(pub(crate) MORSE_MAX_NUM = constants.morse_max_num),
        const_declaration!(pub(crate) MAX_PATTERNS_PER_KEY = constants.max_patterns_per_key),
        format!("pub(crate) const BUILD_HASH: u32 = {build_hash:#010x};\n"),
    ];

    // Add event channel constants
    // Note: with_defaults() has already been called in main(), so all values are Some
    let (ble_state_change_size, ble_state_change_pubs, ble_state_change_subs) = events.ble_state_change.into_values();
    let (ble_profile_change_size, ble_profile_change_pubs, ble_profile_change_subs) =
        events.ble_profile_change.into_values();
    let (connection_change_size, connection_change_pubs, connection_change_subs) =
        events.connection_change.into_values();
    let (key_size, key_pubs, key_subs) = events.key.into_values();
    let (modifier_size, modifier_pubs, modifier_subs) = events.modifier.into_values();
    let (keyboard_size, keyboard_pubs, keyboard_subs) = events.keyboard.into_values();
    let (layer_change_size, layer_change_pubs, layer_change_subs) = events.layer_change.into_values();
    let (wpm_update_size, wpm_update_pubs, wpm_update_subs) = events.wpm_update.into_values();
    let (led_indicator_size, led_indicator_pubs, led_indicator_subs) = events.led_indicator.into_values();
    let (sleep_state_size, sleep_state_pubs, sleep_state_subs) = events.sleep_state.into_values();
    let (battery_state_size, battery_state_pubs, battery_state_subs) = events.battery_state.into_values();
    let (battery_adc_size, battery_adc_pubs, battery_adc_subs) = events.battery_adc.into_values();
    let (charging_state_size, charging_state_pubs, charging_state_subs) = events.charging_state.into_values();
    let (pointing_size, pointing_pubs, pointing_subs) = events.pointing.into_values();
    let (touchpad_size, touchpad_pubs, touchpad_subs) = events.touchpad.into_values();
    let (peripheral_connected_size, peripheral_connected_pubs, peripheral_connected_subs) =
        events.peripheral_connected.into_values();
    let (central_connected_size, central_connected_pubs, central_connected_subs) =
        events.central_connected.into_values();
    let (peripheral_battery_size, peripheral_battery_pubs, peripheral_battery_subs) =
        events.peripheral_battery.into_values();
    let (clear_peer_size, clear_peer_pubs, clear_peer_subs) = events.clear_peer.into_values();

    constant_strs.extend([
        // BLE events
        const_declaration!(pub(crate) BLE_STATE_CHANGE_EVENT_CHANNEL_SIZE = ble_state_change_size),
        const_declaration!(pub(crate) BLE_STATE_CHANGE_EVENT_PUB_SIZE = ble_state_change_pubs),
        const_declaration!(pub(crate) BLE_STATE_CHANGE_EVENT_SUB_SIZE = ble_state_change_subs),
        const_declaration!(pub(crate) BLE_PROFILE_CHANGE_EVENT_CHANNEL_SIZE = ble_profile_change_size),
        const_declaration!(pub(crate) BLE_PROFILE_CHANGE_EVENT_PUB_SIZE = ble_profile_change_pubs),
        const_declaration!(pub(crate) BLE_PROFILE_CHANGE_EVENT_SUB_SIZE = ble_profile_change_subs),
        // Connection events
        const_declaration!(pub(crate) CONNECTION_CHANGE_EVENT_CHANNEL_SIZE = connection_change_size),
        const_declaration!(pub(crate) CONNECTION_CHANGE_EVENT_PUB_SIZE = connection_change_pubs),
        const_declaration!(pub(crate) CONNECTION_CHANGE_EVENT_SUB_SIZE = connection_change_subs),
        // Input events
        const_declaration!(pub(crate) KEY_EVENT_CHANNEL_SIZE = key_size),
        const_declaration!(pub(crate) KEY_EVENT_PUB_SIZE = key_pubs),
        const_declaration!(pub(crate) KEY_EVENT_SUB_SIZE = key_subs),
        const_declaration!(pub(crate) MODIFIER_EVENT_CHANNEL_SIZE = modifier_size),
        const_declaration!(pub(crate) MODIFIER_EVENT_PUB_SIZE = modifier_pubs),
        const_declaration!(pub(crate) MODIFIER_EVENT_SUB_SIZE = modifier_subs),
        const_declaration!(pub(crate) KEYBOARD_EVENT_CHANNEL_SIZE = keyboard_size),
        const_declaration!(pub(crate) KEYBOARD_EVENT_PUB_SIZE = keyboard_pubs),
        const_declaration!(pub(crate) KEYBOARD_EVENT_SUB_SIZE = keyboard_subs),
        // Keyboard state events
        const_declaration!(pub(crate) LAYER_CHANGE_EVENT_CHANNEL_SIZE = layer_change_size),
        const_declaration!(pub(crate) LAYER_CHANGE_EVENT_PUB_SIZE = layer_change_pubs),
        const_declaration!(pub(crate) LAYER_CHANGE_EVENT_SUB_SIZE = layer_change_subs),
        const_declaration!(pub(crate) WPM_UPDATE_EVENT_CHANNEL_SIZE = wpm_update_size),
        const_declaration!(pub(crate) WPM_UPDATE_EVENT_PUB_SIZE = wpm_update_pubs),
        const_declaration!(pub(crate) WPM_UPDATE_EVENT_SUB_SIZE = wpm_update_subs),
        const_declaration!(pub(crate) LED_INDICATOR_EVENT_CHANNEL_SIZE = led_indicator_size),
        const_declaration!(pub(crate) LED_INDICATOR_EVENT_PUB_SIZE = led_indicator_pubs),
        const_declaration!(pub(crate) LED_INDICATOR_EVENT_SUB_SIZE = led_indicator_subs),
        const_declaration!(pub(crate) SLEEP_STATE_EVENT_CHANNEL_SIZE = sleep_state_size),
        const_declaration!(pub(crate) SLEEP_STATE_EVENT_PUB_SIZE = sleep_state_pubs),
        const_declaration!(pub(crate) SLEEP_STATE_EVENT_SUB_SIZE = sleep_state_subs),
        // Power events
        const_declaration!(pub(crate) BATTERY_STATE_EVENT_CHANNEL_SIZE = battery_state_size),
        const_declaration!(pub(crate) BATTERY_STATE_EVENT_PUB_SIZE = battery_state_pubs),
        const_declaration!(pub(crate) BATTERY_STATE_EVENT_SUB_SIZE = battery_state_subs),
        const_declaration!(pub(crate) BATTERY_ADC_EVENT_CHANNEL_SIZE = battery_adc_size),
        const_declaration!(pub(crate) BATTERY_ADC_EVENT_PUB_SIZE = battery_adc_pubs),
        const_declaration!(pub(crate) BATTERY_ADC_EVENT_SUB_SIZE = battery_adc_subs),
        const_declaration!(pub(crate) CHARGING_STATE_EVENT_CHANNEL_SIZE = charging_state_size),
        const_declaration!(pub(crate) CHARGING_STATE_EVENT_PUB_SIZE = charging_state_pubs),
        const_declaration!(pub(crate) CHARGING_STATE_EVENT_SUB_SIZE = charging_state_subs),
        // Pointing device events
        const_declaration!(pub(crate) POINTING_EVENT_CHANNEL_SIZE = pointing_size),
        const_declaration!(pub(crate) POINTING_EVENT_PUB_SIZE = pointing_pubs),
        const_declaration!(pub(crate) POINTING_EVENT_SUB_SIZE = pointing_subs),
        const_declaration!(pub(crate) TOUCHPAD_EVENT_CHANNEL_SIZE = touchpad_size),
        const_declaration!(pub(crate) TOUCHPAD_EVENT_PUB_SIZE = touchpad_pubs),
        const_declaration!(pub(crate) TOUCHPAD_EVENT_SUB_SIZE = touchpad_subs),
        // Split events
        const_declaration!(pub(crate) PERIPHERAL_CONNECTED_EVENT_CHANNEL_SIZE = peripheral_connected_size),
        const_declaration!(pub(crate) PERIPHERAL_CONNECTED_EVENT_PUB_SIZE = peripheral_connected_pubs),
        const_declaration!(pub(crate) PERIPHERAL_CONNECTED_EVENT_SUB_SIZE = peripheral_connected_subs),
        const_declaration!(pub(crate) CENTRAL_CONNECTED_EVENT_CHANNEL_SIZE = central_connected_size),
        const_declaration!(pub(crate) CENTRAL_CONNECTED_EVENT_PUB_SIZE = central_connected_pubs),
        const_declaration!(pub(crate) CENTRAL_CONNECTED_EVENT_SUB_SIZE = central_connected_subs),
        const_declaration!(pub(crate) PERIPHERAL_BATTERY_EVENT_CHANNEL_SIZE = peripheral_battery_size),
        const_declaration!(pub(crate) PERIPHERAL_BATTERY_EVENT_PUB_SIZE = peripheral_battery_pubs),
        const_declaration!(pub(crate) PERIPHERAL_BATTERY_EVENT_SUB_SIZE = peripheral_battery_subs),
        const_declaration!(pub(crate) CLEAR_PEER_EVENT_CHANNEL_SIZE = clear_peer_size),
        const_declaration!(pub(crate) CLEAR_PEER_EVENT_PUB_SIZE = clear_peer_pubs),
        const_declaration!(pub(crate) CLEAR_PEER_EVENT_SUB_SIZE = clear_peer_subs),
    ]);

    constant_strs
        .into_iter()
        .map(|s| "#[allow(clippy::redundant_static_lifetimes)]\n".to_owned() + s.as_str())
        .collect::<Vec<_>>()
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
