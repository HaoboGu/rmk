// Host configuration types

use serde::Deserialize;
use serde_inline_default::serde_inline_default;

/// Configuration for host tools
#[serde_inline_default]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    /// Whether Vial is enabled
    #[serde_inline_default(true)]
    pub vial_enabled: bool,
    /// Unlock keys for Vial (optional)
    pub unlock_keys: Option<Vec<[u8; 2]>>,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            vial_enabled: true,
            unlock_keys: None,
        }
    }
}
