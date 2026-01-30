// RMK constants configuration types

use serde::{de, Deserialize as SerdeDeserialize};
use serde_derive::Deserialize;
use serde_inline_default::serde_inline_default;

/// Keyboard constants configuration for performance and hardware limits
#[serde_inline_default]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RmkConstantsConfig {
    /// Mouse key interval (ms) - controls mouse movement speed
    #[serde_inline_default(20)]
    pub mouse_key_interval: u32,
    /// Mouse wheel interval (ms) - controls scrolling speed
    #[serde_inline_default(80)]
    pub mouse_wheel_interval: u32,
    /// Maximum number of combos keyboard can store
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_combo_max_num")]
    pub combo_max_num: usize,
    /// Maximum number of keys pressed simultaneously in a combo
    #[serde_inline_default(4)]
    pub combo_max_length: usize,
    /// Maximum number of forks for conditional key actions
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_fork_max_num")]
    pub fork_max_num: usize,
    /// Maximum number of morses keyboard can store
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_morse_max_num")]
    pub morse_max_num: usize,
    /// Maximum number of patterns a morse key can handle
    #[serde_inline_default(8)]
    #[serde(deserialize_with = "check_max_patterns_per_key")]
    pub max_patterns_per_key: usize,
    /// Macro space size in bytes for storing sequences
    #[serde_inline_default(256)]
    pub macro_space_size: usize,
    /// Default debounce time in ms
    #[serde_inline_default(20)]
    pub debounce_time: u16,
    /// Event channel size
    #[serde_inline_default(16)]
    pub event_channel_size: usize,
    /// Report channel size
    #[serde_inline_default(16)]
    pub report_channel_size: usize,
    /// Vial channel size
    #[serde_inline_default(4)]
    pub vial_channel_size: usize,
    /// Flash channel size
    #[serde_inline_default(4)]
    pub flash_channel_size: usize,
    /// The number of the split peripherals
    #[serde_inline_default(1)]
    pub split_peripherals_num: usize,
    /// The number of available BLE profiles
    #[serde_inline_default(3)]
    pub ble_profiles_num: usize,
    /// BLE Split Central sleep timeout in minutes (0 = disabled)
    #[serde_inline_default(0)]
    pub split_central_sleep_timeout_seconds: u32,
}

fn check_combo_max_num<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if value > 256 {
        panic!("❌ Parse `keyboard.toml` error: combo_max_num must be between 0 and 256, got {value}");
    }
    Ok(value)
}

fn check_morse_max_num<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if value > 256 {
        panic!("❌ Parse `keyboard.toml` error: morse_max_num must be between 0 and 256, got {value}");
    }
    Ok(value)
}

fn check_max_patterns_per_key<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if !(4..=65536).contains(&value) {
        panic!("❌ Parse `keyboard.toml` error: max_patterns_per_key must be between 4 and 65566, got {value}");
    }
    Ok(value)
}

fn check_fork_max_num<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = SerdeDeserialize::deserialize(deserializer)?;
    if value > 256 {
        panic!("❌ Parse `keyboard.toml` error: fork_max_num must be between 0 and 256, got {value}");
    }
    Ok(value)
}

/// This separate Default impl is needed when `[rmk]` section is not set in keyboard.toml
impl Default for RmkConstantsConfig {
    fn default() -> Self {
        Self {
            mouse_key_interval: 20,
            mouse_wheel_interval: 80,
            combo_max_num: 8,
            combo_max_length: 4,
            fork_max_num: 8,
            morse_max_num: 8,
            max_patterns_per_key: 8,
            macro_space_size: 256,
            debounce_time: 20,
            event_channel_size: 16,
            report_channel_size: 16,
            vial_channel_size: 4,
            flash_channel_size: 4,
            split_peripherals_num: 0,
            ble_profiles_num: 3,
            split_central_sleep_timeout_seconds: 0,
        }
    }
}
