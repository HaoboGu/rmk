// Light API implementations

use crate::types::LightConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get light configuration
    /// This is renamed from get_light_config
    pub fn light(&self) -> LightConfig {
        // Delegate to the existing implementation in light.rs
        self.get_light_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in light.rs
}
