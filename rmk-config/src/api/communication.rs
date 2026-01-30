// Communication API implementations

use crate::communication::CommunicationConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get communication configuration
    /// This is renamed from get_communication_config
    pub fn communication(&self) -> Result<CommunicationConfig, String> {
        // Delegate to the existing implementation in communication.rs
        self.get_communication_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in communication.rs
}
