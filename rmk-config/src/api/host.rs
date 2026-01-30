// Host API implementations

use crate::types::HostConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get host configuration
    /// This is renamed from get_host_config
    pub fn host(&self) -> HostConfig {
        // Delegate to the existing implementation in host.rs
        self.get_host_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in host.rs
}
