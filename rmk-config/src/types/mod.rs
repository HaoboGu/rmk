// Type definitions for rmk-config
//
// This module contains all the struct definitions used for configuration.
// Types are organized by their functional domain.

pub mod behavior;
pub mod common;
pub mod communication;
pub mod constants;
pub mod events;
pub mod hardware;
pub mod host;
pub mod input_device;
pub mod keyboard;
pub mod layout;
pub mod light;
pub mod matrix;
pub mod split;
pub mod storage;

// Re-export commonly used types
pub use behavior::*;
pub use common::*;
pub use communication::*;
pub use constants::*;
pub use events::*;
pub use hardware::*;
pub use host::*;
pub use input_device::*;
pub use keyboard::*;
pub use layout::*;
pub use light::*;
pub use matrix::*;
pub use split::*;
pub use storage::*;
