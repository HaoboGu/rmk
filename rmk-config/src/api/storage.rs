// Storage API implementations

use crate::types::StorageConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get storage configuration
    /// This is renamed from get_storage_config
    pub fn storage(&self) -> StorageConfig {
        // Delegate to the existing implementation in storage.rs
        self.get_storage_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in storage.rs
}
