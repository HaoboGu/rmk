use crate::{DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS, MIN_PASSKEY_ENTRY_TIMEOUT_SECS};

/// Compile-time constants emitted as `pub const` items by `rmk-types/build.rs`.
pub struct BuildConstants {
    pub combo_max_num: usize,
    pub combo_max_length: usize,
    pub fork_max_num: usize,
    pub morse_max_num: usize,
    pub max_patterns_per_key: usize,
    pub macro_space_size: usize,
    pub debounce_time: u16,
    pub mouse_key_interval: u16,
    pub mouse_wheel_interval: u16,
    pub report_channel_size: usize,
    pub vial_channel_size: usize,
    pub flash_channel_size: usize,
    pub split_peripherals_num: usize,
    pub ble_profiles_num: usize,
    pub split_central_sleep_timeout_seconds: u32,
    pub events: Vec<EventChannel>,
    pub passkey: Option<Passkey>,
}

pub struct EventChannel {
    pub name: String,
    pub channel_size: usize,
    pub pubs: usize,
    pub subs: usize,
}

pub struct Passkey {
    pub enabled: bool,
    pub timeout_secs: u32,
}

impl crate::KeyboardTomlConfig {
    /// Build compile-time constants from the configuration.
    pub fn build_constants(&self) -> Result<BuildConstants, String> {
        let rmk = &self.rmk;

        // Fix split_peripherals_num: when split feature is enabled, ensure at least 1
        let split_peripherals_num = if std::env::var("CARGO_FEATURE_SPLIT").is_ok() && rmk.split_peripherals_num < 1 {
            1
        } else {
            rmk.split_peripherals_num
        };

        // Build event channels
        macro_rules! event_channels {
            ($($field:ident),* $(,)?) => {
                vec![$(
                    EventChannel {
                        name: stringify!($field).to_string(),
                        channel_size: self.event.$field.channel_size,
                        pubs: self.event.$field.pubs,
                        subs: self.event.$field.subs,
                    },
                )*]
            };
        }

        let events = event_channels!(
            ble_status_change,
            connection_change,
            modifier,
            keyboard,
            layer_change,
            wpm_update,
            led_indicator,
            sleep_state,
            battery_state,
            battery_adc,
            charging_state,
            pointing,
            peripheral_connected,
            central_connected,
            peripheral_battery,
            clear_peer,
            action,
        );

        // Build passkey config
        let passkey = self.ble.as_ref().map(resolve_passkey).transpose()?;

        Ok(BuildConstants {
            combo_max_num: rmk.combo_max_num,
            combo_max_length: rmk.combo_max_length,
            fork_max_num: rmk.fork_max_num,
            morse_max_num: rmk.morse_max_num,
            max_patterns_per_key: rmk.max_patterns_per_key,
            macro_space_size: rmk.macro_space_size,
            debounce_time: rmk.debounce_time,
            mouse_key_interval: rmk.mouse_key_interval,
            mouse_wheel_interval: rmk.mouse_wheel_interval,
            report_channel_size: rmk.report_channel_size,
            vial_channel_size: rmk.vial_channel_size,
            flash_channel_size: rmk.flash_channel_size,
            split_peripherals_num,
            ble_profiles_num: rmk.ble_profiles_num,
            split_central_sleep_timeout_seconds: rmk.split_central_sleep_timeout_seconds,
            events,
            passkey,
        })
    }
}

fn resolve_passkey(ble: &crate::BleConfig) -> Result<Passkey, String> {
    let enabled = ble.passkey_entry.unwrap_or(false);
    let timeout_secs = ble.passkey_entry_timeout.unwrap_or(DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS);
    if timeout_secs < MIN_PASSKEY_ENTRY_TIMEOUT_SECS {
        return Err(format!(
            "keyboard.toml: [ble.passkey_entry_timeout] must be at least {} seconds, got {}",
            MIN_PASSKEY_ENTRY_TIMEOUT_SECS, timeout_secs
        ));
    }
    Ok(Passkey { enabled, timeout_secs })
}
