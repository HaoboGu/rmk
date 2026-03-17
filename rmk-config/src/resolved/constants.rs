use crate::{DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS, MIN_PASSKEY_ENTRY_TIMEOUT_SECS};

/// Compile-time constants emitted as `pub const` items by rmk-types/build.rs.
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
    pub fn build_constants(&self) -> BuildConstants {
        let rmk = &self.rmk;

        // Fix split_peripherals_num: when split feature is enabled, ensure at least 1
        let split_peripherals_num = if std::env::var("CARGO_FEATURE_SPLIT").is_ok() && rmk.split_peripherals_num < 1 {
            1
        } else {
            rmk.split_peripherals_num
        };

        // Build event channels
        let events = vec![
            EventChannel {
                name: "ble_status_change".to_string(),
                channel_size: self.event.ble_status_change.channel_size,
                pubs: self.event.ble_status_change.pubs,
                subs: self.event.ble_status_change.subs,
            },
            EventChannel {
                name: "connection_change".to_string(),
                channel_size: self.event.connection_change.channel_size,
                pubs: self.event.connection_change.pubs,
                subs: self.event.connection_change.subs,
            },
            EventChannel {
                name: "modifier".to_string(),
                channel_size: self.event.modifier.channel_size,
                pubs: self.event.modifier.pubs,
                subs: self.event.modifier.subs,
            },
            EventChannel {
                name: "keyboard".to_string(),
                channel_size: self.event.keyboard.channel_size,
                pubs: self.event.keyboard.pubs,
                subs: self.event.keyboard.subs,
            },
            EventChannel {
                name: "layer_change".to_string(),
                channel_size: self.event.layer_change.channel_size,
                pubs: self.event.layer_change.pubs,
                subs: self.event.layer_change.subs,
            },
            EventChannel {
                name: "wpm_update".to_string(),
                channel_size: self.event.wpm_update.channel_size,
                pubs: self.event.wpm_update.pubs,
                subs: self.event.wpm_update.subs,
            },
            EventChannel {
                name: "led_indicator".to_string(),
                channel_size: self.event.led_indicator.channel_size,
                pubs: self.event.led_indicator.pubs,
                subs: self.event.led_indicator.subs,
            },
            EventChannel {
                name: "sleep_state".to_string(),
                channel_size: self.event.sleep_state.channel_size,
                pubs: self.event.sleep_state.pubs,
                subs: self.event.sleep_state.subs,
            },
            EventChannel {
                name: "battery_state".to_string(),
                channel_size: self.event.battery_state.channel_size,
                pubs: self.event.battery_state.pubs,
                subs: self.event.battery_state.subs,
            },
            EventChannel {
                name: "battery_adc".to_string(),
                channel_size: self.event.battery_adc.channel_size,
                pubs: self.event.battery_adc.pubs,
                subs: self.event.battery_adc.subs,
            },
            EventChannel {
                name: "charging_state".to_string(),
                channel_size: self.event.charging_state.channel_size,
                pubs: self.event.charging_state.pubs,
                subs: self.event.charging_state.subs,
            },
            EventChannel {
                name: "pointing".to_string(),
                channel_size: self.event.pointing.channel_size,
                pubs: self.event.pointing.pubs,
                subs: self.event.pointing.subs,
            },
            EventChannel {
                name: "peripheral_connected".to_string(),
                channel_size: self.event.peripheral_connected.channel_size,
                pubs: self.event.peripheral_connected.pubs,
                subs: self.event.peripheral_connected.subs,
            },
            EventChannel {
                name: "central_connected".to_string(),
                channel_size: self.event.central_connected.channel_size,
                pubs: self.event.central_connected.pubs,
                subs: self.event.central_connected.subs,
            },
            EventChannel {
                name: "peripheral_battery".to_string(),
                channel_size: self.event.peripheral_battery.channel_size,
                pubs: self.event.peripheral_battery.pubs,
                subs: self.event.peripheral_battery.subs,
            },
            EventChannel {
                name: "clear_peer".to_string(),
                channel_size: self.event.clear_peer.channel_size,
                pubs: self.event.clear_peer.pubs,
                subs: self.event.clear_peer.subs,
            },
        ];

        // Build passkey config
        let passkey = self.ble.as_ref().map(|ble| {
            let enabled = ble.passkey_entry.unwrap_or(false);
            let timeout_secs = ble.passkey_entry_timeout.unwrap_or(DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS);
            if timeout_secs < MIN_PASSKEY_ENTRY_TIMEOUT_SECS {
                panic!(
                    "passkey_entry_timeout must be at least {} seconds, got {}",
                    MIN_PASSKEY_ENTRY_TIMEOUT_SECS, timeout_secs
                );
            }
            Passkey { enabled, timeout_secs }
        });

        BuildConstants {
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
        }
    }
}
