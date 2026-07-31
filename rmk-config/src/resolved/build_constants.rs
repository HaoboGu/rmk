use serde::Deserialize;

use crate::{DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS, MIN_PASSKEY_ENTRY_TIMEOUT_SECS};

const SUBSCRIBER_DEFAULT_CONFIG: &str = include_str!("../default_config/subscriber_default.toml");

/// Parsed representation of `subscriber_default.toml`.
#[derive(Deserialize)]
struct SubscriberConfig {
    subscriber: Vec<SubscriberEntry>,
}

/// A single entry: bump `subs` for each listed event when all `features` are enabled.
#[derive(Deserialize)]
struct SubscriberEntry {
    features: Vec<String>,
    events: Vec<SubscriberEventEntry>,
}

/// Per-event subscriber bump. `count` defaults to 1.
#[derive(Deserialize)]
struct SubscriberEventEntry {
    name: String,
    #[serde(default = "default_sub_count")]
    count: usize,
}

fn default_sub_count() -> usize {
    1
}

/// Compile-time constants emitted as `pub const` items by `rmk-types/build.rs`.
pub struct BuildConstants {
    pub combo_max_num: usize,
    pub combo_max_length: usize,
    pub fork_max_num: usize,
    pub morse_max_num: usize,
    pub sticky_key_profile_max_num: usize,
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
    pub protocol_max_bulk_size: usize,
    pub protocol_macro_chunk_size: usize,
    pub auto_mouse_layer_max_num: usize,
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
    ///
    /// `active_features` contains feature names enabled on the
    /// **downstream crate** (e.g. `["split", "_ble"]`). These are matched
    /// against `subscriber_default.toml` to auto-bump event subscriber counts.
    pub fn build_constants(&self, active_features: &[&str]) -> Result<BuildConstants, String> {
        let rmk = &self.rmk;

        // Fix split_peripherals_num: when split feature is enabled, ensure at least 1
        let split_peripherals_num = if active_features.contains(&"split") && rmk.split_peripherals_num < 1 {
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

        let mut events = event_channels!(
            connection_status_change,
            modifier,
            keyboard,
            layer_change,
            layer_transition,
            wpm_update,
            led_indicator,
            sleep_state,
            battery_status,
            battery_adc,
            charging_state,
            pointing,
            peripheral_connected,
            central_connected,
            peripheral_battery,
            clear_peer,
            dfu_status,
            action,
        );

        // Auto-bump subscriber counts based on enabled feature flags.
        // Declarations live in subscriber_default.toml.
        apply_feature_subscriber_bumps(&mut events, active_features);

        // Only validate passkey settings when the build will emit passkey constants.
        let passkey = if active_features.contains(&"passkey_entry") {
            self.ble.as_ref().map(resolve_passkey_enabled).transpose()?
        } else {
            None
        };

        // Validate that config values do not exceed protocol ceilings.
        use crate::protocol_limits;
        if rmk.combo_max_length > protocol_limits::MAX_COMBO_SIZE {
            return Err(format!(
                "combo_max_length ({}) exceeds protocol ceiling MAX_COMBO_SIZE ({})",
                rmk.combo_max_length,
                protocol_limits::MAX_COMBO_SIZE
            ));
        }
        if rmk.max_patterns_per_key > protocol_limits::MAX_MORSE_SIZE {
            return Err(format!(
                "max_patterns_per_key ({}) exceeds protocol ceiling MAX_MORSE_SIZE ({})",
                rmk.max_patterns_per_key,
                protocol_limits::MAX_MORSE_SIZE
            ));
        }
        if rmk.protocol_macro_chunk_size > protocol_limits::MAX_MACRO_DATA_SIZE {
            return Err(format!(
                "protocol_macro_chunk_size ({}) exceeds protocol ceiling MAX_MACRO_DATA_SIZE ({})",
                rmk.protocol_macro_chunk_size,
                protocol_limits::MAX_MACRO_DATA_SIZE
            ));
        }
        if rmk.protocol_max_bulk_size > protocol_limits::MAX_BULK_SIZE {
            return Err(format!(
                "protocol_max_bulk_size ({}) exceeds protocol ceiling MAX_BULK_SIZE ({})",
                rmk.protocol_max_bulk_size,
                protocol_limits::MAX_BULK_SIZE
            ));
        }

        let auto_mouse_layer_max_num = rmk
            .auto_mouse_layer_max_num
            .unwrap_or(crate::resolved::behavior::DEFAULT_AUTO_MOUSE_LAYER_MAX_NUM);
        let sticky_key_profile_max_num = self.sticky_key_profile_capacity();
        let sticky_key_profile_count = self.configured_sticky_key_profile_count();
        if sticky_key_profile_count > sticky_key_profile_max_num {
            return Err(format!(
                "behavior.sticky_key.profiles defines {sticky_key_profile_count} profiles, but `[rmk] sticky_key_profile_max_num` is {sticky_key_profile_max_num}. Raise it in keyboard.toml"
            ));
        }
        if let Some(entries) = self.behavior.as_ref().and_then(|b| b.auto_mouse_layer.as_ref()) {
            if entries.len() > auto_mouse_layer_max_num {
                return Err(format!(
                    "number of [[behavior.auto_mouse_layer]] entries ({}) exceeds auto_mouse_layer_max_num ({})",
                    entries.len(),
                    auto_mouse_layer_max_num
                ));
            }
            let uses_action_event = entries
                .iter()
                .any(|e| e.deactivate_on_key == Some(true) || e.reset_timeout_on_key == Some(true));
            if uses_action_event && events.iter().any(|e| e.name == "action" && e.subs == 0) {
                return Err(
                    "[[behavior.auto_mouse_layer]].deactivate_on_key / reset_timeout_on_key require [event.action] subs to be at least 1".to_string(),
                );
            }
        }

        // The keyboard always subscribes to this internal event. Auto-mouse
        // additionally publishes it when configured, so validate the generated
        // pubsub resources at their configuration boundary rather than panicking
        // during runtime initialization.
        let layer_transition = events
            .iter()
            .find(|event| event.name == "layer_transition")
            .expect("layer_transition is a built-in event");
        if layer_transition.channel_size == 0 {
            return Err("[event.layer_transition].channel_size must be at least 1".to_string());
        }
        if layer_transition.subs == 0 {
            return Err(
                "[event.layer_transition].subs must be at least 1 because Keyboard subscribes to it".to_string(),
            );
        }
        let auto_mouse_configured = self
            .behavior
            .as_ref()
            .and_then(|behavior| behavior.auto_mouse_layer.as_ref())
            .is_some_and(|entries| !entries.is_empty());
        if auto_mouse_configured && layer_transition.pubs == 0 {
            return Err(
                "[event.layer_transition].pubs must be at least 1 when [[behavior.auto_mouse_layer]] is configured"
                    .to_string(),
            );
        }

        Ok(BuildConstants {
            combo_max_num: rmk.combo_max_num,
            combo_max_length: rmk.combo_max_length,
            fork_max_num: rmk.fork_max_num,
            morse_max_num: rmk.morse_max_num,
            sticky_key_profile_max_num,
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
            protocol_max_bulk_size: rmk.protocol_max_bulk_size,
            protocol_macro_chunk_size: rmk.protocol_macro_chunk_size,
            auto_mouse_layer_max_num,
            events,
            passkey,
        })
    }
}

/// Bump event subscriber counts based on feature flags declared in `subscriber_default.toml`.
///
/// `active_features` contains lowercase feature names (e.g. `"split"`, `"_ble"`).
fn apply_feature_subscriber_bumps(events: &mut [EventChannel], active_features: &[&str]) {
    let sub_config: SubscriberConfig =
        toml::from_str(SUBSCRIBER_DEFAULT_CONFIG).expect("Failed to parse subscriber_default.toml");

    for entry in &sub_config.subscriber {
        let all_enabled = entry.features.iter().all(|f| active_features.contains(&f.as_str()));
        if all_enabled {
            for sub_event in &entry.events {
                if let Some(event) = events.iter_mut().find(|e| e.name == sub_event.name) {
                    event.subs += sub_event.count;
                } else {
                    println!(
                        "cargo:warning=subscriber_default.toml: unknown event \"{}\"",
                        sub_event.name
                    );
                }
            }
        }
    }
}

fn resolve_passkey_enabled(ble: &crate::BleConfig) -> Result<Passkey, String> {
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

#[cfg(test)]
mod tests {
    use super::{BuildConstants, resolve_passkey_enabled};
    use crate::{BleConfig, DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS, MIN_PASSKEY_ENTRY_TIMEOUT_SECS};

    #[test]
    fn validates_passkey_timeout() {
        let ble = BleConfig {
            passkey_entry_timeout: Some(MIN_PASSKEY_ENTRY_TIMEOUT_SECS - 1),
            ..Default::default()
        };

        let err = match resolve_passkey_enabled(&ble) {
            Ok(_) => panic!("expected passkey timeout validation failure"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            format!(
                "keyboard.toml: [ble.passkey_entry_timeout] must be at least {} seconds, got {}",
                MIN_PASSKEY_ENTRY_TIMEOUT_SECS,
                MIN_PASSKEY_ENTRY_TIMEOUT_SECS - 1
            )
        );
    }

    #[test]
    fn uses_default_timeout() {
        let ble = BleConfig::default();
        let passkey = resolve_passkey_enabled(&ble).unwrap();

        assert!(!passkey.enabled);
        assert_eq!(passkey.timeout_secs, DEFAULT_PASSKEY_ENTRY_TIMEOUT_SECS);
    }

    fn parse(toml: &str) -> crate::KeyboardTomlConfig {
        toml::from_str(toml).expect("Failed to parse keyboard config")
    }

    #[test]
    fn auto_mouse_layer_max_num_explicitly_too_small_is_rejected() {
        let toml = "[rmk]\nauto_mouse_layer_max_num = 0\n\n[[behavior.auto_mouse_layer]]\ntarget_layer = 1\n";
        let err = match parse(toml).build_constants(&[]) {
            Ok(_) => panic!("expected auto_mouse_layer_max_num validation failure"),
            Err(err) => err,
        };
        assert!(err.contains("auto_mouse_layer_max_num"));
    }

    #[test]
    fn auto_mouse_layer_within_capacity_is_accepted() {
        let toml = "[rmk]\nauto_mouse_layer_max_num = 1\n\n[[behavior.auto_mouse_layer]]\ntarget_layer = 1\nextra_mouse_keys = [\"LCtrl\"]\n";
        assert!(parse(toml).build_constants(&[]).is_ok());
    }

    #[test]
    fn sticky_key_profile_capacity_is_derived_from_configuration() {
        let toml = r#"
[behavior.sticky_key.profiles.one]
[behavior.sticky_key.profiles.two]
[behavior.sticky_key.profiles.three]
[behavior.sticky_key.profiles.four]
[behavior.sticky_key.profiles.five]
"#;
        let constants = parse(toml).build_constants(&[]).unwrap();
        assert_eq!(constants.sticky_key_profile_max_num, 5);
    }

    #[test]
    fn sticky_key_profile_capacity_can_be_explicitly_disabled() {
        let constants = parse("[rmk]\nsticky_key_profile_max_num = 0\n")
            .build_constants(&[])
            .unwrap();
        assert_eq!(constants.sticky_key_profile_max_num, 0);
    }

    #[test]
    fn sticky_key_profile_capacity_rejects_too_small_override() {
        let toml = "[rmk]\nsticky_key_profile_max_num = 0\n\n[behavior.sticky_key.profiles.named]\n";
        let err = match parse(toml).build_constants(&[]) {
            Ok(_) => panic!("expected sticky_key_profile_max_num validation failure"),
            Err(err) => err,
        };
        assert!(err.contains("sticky_key_profile_max_num"));
    }

    #[test]
    fn deactivate_on_key_without_action_subs_is_rejected() {
        let toml = "[[behavior.auto_mouse_layer]]\ntarget_layer = 1\ndeactivate_on_key = true\n";
        let err = match parse(toml).build_constants(&[]) {
            Ok(_) => panic!("expected action subs validation failure"),
            Err(err) => err,
        };
        assert!(err.contains("[event.action]"));
    }

    #[test]
    fn deactivate_on_key_with_action_subs_set_is_accepted() {
        let toml = "[event.action]\nchannel_size = 16\npubs = 1\nsubs = 1\n\n[[behavior.auto_mouse_layer]]\ntarget_layer = 1\ndeactivate_on_key = true\n";
        assert!(parse(toml).build_constants(&[]).is_ok());
    }

    #[test]
    fn layer_transition_requires_keyboard_resources() {
        for field in ["channel_size", "subs"] {
            let toml = if field == "channel_size" {
                "[event.layer_transition]\nchannel_size = 0\npubs = 1\nsubs = 1\n"
            } else {
                "[event.layer_transition]\nchannel_size = 2\npubs = 1\nsubs = 0\n"
            };
            let err = match parse(&toml).build_constants(&[]) {
                Err(err) => err,
                Ok(_) => panic!("expected layer-transition validation failure"),
            };
            assert!(err.contains("[event.layer_transition]"));
            assert!(err.contains(field));
        }
    }

    #[test]
    fn auto_mouse_requires_layer_transition_publisher() {
        let toml = "[event.layer_transition]\nchannel_size = 1\npubs = 0\nsubs = 1\n\n[[behavior.auto_mouse_layer]]\ntarget_layer = 1\n";
        let err = match parse(toml).build_constants(&[]) {
            Err(err) => err,
            Ok(_) => panic!("expected layer-transition publisher validation failure"),
        };
        assert!(err.contains("[event.layer_transition].pubs"));
    }

    #[test]
    fn ble_reserves_advertising_timeout_wake_subscribers() {
        // ble/mod.rs subscribes to KeyboardEvent/PointingEvent when advertising
        // times out, on top of every permanent subscriber. Without a reserved
        // slot that call panics instead of sleeping until the next key press.
        let base = parse("").build_constants(&[]).unwrap();
        let ble = parse("").build_constants(&["_ble"]).unwrap();

        let subs =
            |constants: &BuildConstants, event: &str| constants.events.iter().find(|e| e.name == event).unwrap().subs;
        for event in ["keyboard", "pointing"] {
            assert_eq!(
                subs(&ble, event),
                subs(&base, event) + 1,
                "{event} needs a wake subscriber slot under _ble"
            );
        }
    }
}
