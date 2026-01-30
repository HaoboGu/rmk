// Event configuration types

use serde::Deserialize;

/// Event channel configuration for a single event type
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventChannelConfig {
    /// Channel buffer size
    #[serde(default)]
    pub channel_size: Option<usize>,
    /// Number of publishers
    #[serde(default)]
    pub pubs: Option<usize>,
    /// Number of subscribers
    #[serde(default)]
    pub subs: Option<usize>,
}

impl EventChannelConfig {
    /// Merge with defaults: user config takes precedence, fallback to defaults for None fields
    pub fn with_defaults(mut self, defaults: EventChannelConfig) -> Self {
        self.channel_size = self.channel_size.or(defaults.channel_size);
        self.pubs = self.pubs.or(defaults.pubs);
        self.subs = self.subs.or(defaults.subs);
        self
    }

    /// Extract final values (all fields must be Some at this point)
    pub fn into_values(self) -> (usize, usize, usize) {
        (
            self.channel_size.expect("channel_size must be set after with_defaults"),
            self.pubs.expect("pubs must be set after with_defaults"),
            self.subs.expect("subs must be set after with_defaults"),
        )
    }
}

// Default event configurations with semantic names

/// Default for simple events: (1, 1, 1)
fn default_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(1),
        pubs: Some(1),
        subs: Some(1),
    }
}

/// Default for monitored events
fn default_monitored_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(1),
        pubs: Some(1),
        subs: Some(4),
    }
}

/// Default for buffered multi-monitored events: (2, 1, 4)
fn default_led_indicator_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(2),
        pubs: Some(1),
        subs: Some(4),
    }
}

/// Default for high-frequency input events: (8, 1, 2)
fn default_input_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(8),
        pubs: Some(1),
        subs: Some(2),
    }
}

/// Default for BLE state change event: (2, 1, 1)
fn default_ble_state_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(2),
        pubs: Some(1),
        subs: Some(1),
    }
}

/// Default for peripheral battery monitoring: (2, 1, 2)
fn default_peripheral_battery_event() -> EventChannelConfig {
    EventChannelConfig {
        channel_size: Some(2),
        pubs: Some(1),
        subs: Some(2),
    }
}

/// Event configuration for all controller events
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventConfig {
    #[serde(default = "default_ble_state_event")]
    pub ble_state_change: EventChannelConfig,
    #[serde(default = "default_event")]
    pub ble_profile_change: EventChannelConfig,
    #[serde(default = "default_event")]
    pub connection_change: EventChannelConfig,
    #[serde(default = "default_input_event")]
    pub key: EventChannelConfig,
    #[serde(default = "default_input_event")]
    pub modifier: EventChannelConfig,
    #[serde(default = "default_monitored_event")]
    pub layer_change: EventChannelConfig,
    #[serde(default = "default_event")]
    pub wpm_update: EventChannelConfig,
    #[serde(default = "default_led_indicator_event")]
    pub led_indicator: EventChannelConfig,
    #[serde(default = "default_monitored_event")]
    pub sleep_state: EventChannelConfig,
    #[serde(default = "default_monitored_event")]
    pub battery_level: EventChannelConfig,
    #[serde(default = "default_monitored_event")]
    pub charging_state: EventChannelConfig,
    #[serde(default = "default_event")]
    pub peripheral_connected: EventChannelConfig,
    #[serde(default = "default_event")]
    pub central_connected: EventChannelConfig,
    #[serde(default = "default_peripheral_battery_event")]
    pub peripheral_battery: EventChannelConfig,
    #[serde(default = "default_monitored_event")]
    pub clear_peer: EventChannelConfig,
}

impl EventConfig {
    pub fn with_defaults(mut self) -> Self {
        self.ble_state_change = self.ble_state_change.with_defaults(default_ble_state_event());
        self.ble_profile_change = self.ble_profile_change.with_defaults(default_event());
        self.connection_change = self.connection_change.with_defaults(default_event());
        self.key = self.key.with_defaults(default_input_event());
        self.modifier = self.modifier.with_defaults(default_input_event());
        self.layer_change = self.layer_change.with_defaults(default_monitored_event());
        self.wpm_update = self.wpm_update.with_defaults(default_event());
        self.led_indicator = self.led_indicator.with_defaults(default_led_indicator_event());
        self.sleep_state = self.sleep_state.with_defaults(default_monitored_event());
        self.battery_level = self.battery_level.with_defaults(default_monitored_event());
        self.charging_state = self.charging_state.with_defaults(default_monitored_event());
        self.peripheral_connected = self.peripheral_connected.with_defaults(default_event());
        self.central_connected = self.central_connected.with_defaults(default_event());
        self.peripheral_battery = self.peripheral_battery.with_defaults(default_peripheral_battery_event());
        self.clear_peer = self.clear_peer.with_defaults(default_monitored_event());
        self
    }
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            ble_state_change: default_ble_state_event(),
            ble_profile_change: default_event(),
            connection_change: default_event(),
            key: default_input_event(),
            modifier: default_input_event(),
            layer_change: default_monitored_event(),
            wpm_update: default_event(),
            led_indicator: default_led_indicator_event(),
            sleep_state: default_monitored_event(),
            battery_level: default_monitored_event(),
            charging_state: default_monitored_event(),
            peripheral_connected: default_event(),
            central_connected: default_event(),
            peripheral_battery: default_peripheral_battery_event(),
            clear_peer: default_monitored_event(),
        }
    }
}
