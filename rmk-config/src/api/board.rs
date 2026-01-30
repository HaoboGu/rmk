// Board API implementations

use crate::board::BoardConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get board configuration
    /// This is renamed from get_board_config
    pub fn board(&self) -> Result<BoardConfig, String> {
        // Delegate to the existing implementation in board.rs
        self.get_board_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in board.rs
}
