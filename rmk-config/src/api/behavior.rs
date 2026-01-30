// Behavior API implementations

use crate::types::BehaviorConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get behavior configuration
    /// This is renamed from get_behavior_config
    pub fn behavior(&self) -> Result<BehaviorConfig, String> {
        // Delegate to the existing implementation in behavior.rs
        self.get_behavior_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in behavior.rs
}
