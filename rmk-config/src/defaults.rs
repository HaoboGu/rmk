//! Centralized default values for RMK configuration
//!
//! This module provides all default values used in keyboard configuration.
//! Having them in one place makes it easier to maintain and document.

// ============================================================================
// RMK Constants Defaults
// ============================================================================

/// Default mouse key interval in milliseconds - controls mouse movement speed
pub const MOUSE_KEY_INTERVAL_MS: u32 = 20;

/// Default mouse wheel interval in milliseconds - controls scrolling speed
pub const MOUSE_WHEEL_INTERVAL_MS: u32 = 80;

/// Default maximum number of combos keyboard can store
pub const COMBO_MAX_NUM: usize = 8;

/// Default maximum number of keys pressed simultaneously in a combo
pub const COMBO_MAX_LENGTH: usize = 4;

/// Default maximum number of forks for conditional key actions
pub const FORK_MAX_NUM: usize = 8;

/// Default maximum number of morses keyboard can store
pub const MORSE_MAX_NUM: usize = 8;

/// Default maximum number of patterns a morse key can handle
pub const MAX_PATTERNS_PER_KEY: usize = 8;

/// Default macro space size in bytes for storing sequences
pub const MACRO_SPACE_SIZE: usize = 256;

/// Default debounce time in milliseconds
pub const DEBOUNCE_TIME_MS: u16 = 20;

/// Default event channel size
pub const EVENT_CHANNEL_SIZE: usize = 16;

/// Default report channel size
pub const REPORT_CHANNEL_SIZE: usize = 16;

/// Default vial channel size
pub const VIAL_CHANNEL_SIZE: usize = 4;

/// Default flash channel size
pub const FLASH_CHANNEL_SIZE: usize = 4;

/// Default number of split peripherals
pub const SPLIT_PERIPHERALS_NUM: usize = 0;

/// Default number of available BLE profiles
pub const BLE_PROFILES_NUM: usize = 3;

/// Default BLE split central sleep timeout in seconds (0 = disabled)
pub const SPLIT_CENTRAL_SLEEP_TIMEOUT_SECONDS: u32 = 0;

// ============================================================================
// Device Info Defaults
// ============================================================================

/// Default vendor ID
pub const DEFAULT_VID: u16 = 0xE118;

/// Default product ID
pub const DEFAULT_PID: u16 = 0x0001;

/// Default manufacturer name
pub const DEFAULT_MANUFACTURER: &str = "RMK";

/// Default product name
pub const DEFAULT_PRODUCT_NAME: &str = "RMK Keyboard";

/// Default serial number (Vial format)
pub const DEFAULT_SERIAL_NUMBER: &str = "vial:f64c2b3c:000001";

// ============================================================================
// Input Device Defaults
// ============================================================================

/// Default PMW3610 report rate in Hz
pub const PMW3610_REPORT_HZ: u16 = 125;

/// Default encoder resolution
pub const ENCODER_RESOLUTION: u8 = 4;

// ============================================================================
// Validation Limits
// ============================================================================

/// Maximum allowed value for combo_max_num
pub const COMBO_MAX_NUM_LIMIT: usize = 256;

/// Maximum allowed value for fork_max_num
pub const FORK_MAX_NUM_LIMIT: usize = 256;

/// Maximum allowed value for morse_max_num
pub const MORSE_MAX_NUM_LIMIT: usize = 256;

/// Minimum allowed value for max_patterns_per_key
pub const MAX_PATTERNS_PER_KEY_MIN: usize = 4;

/// Maximum allowed value for max_patterns_per_key
pub const MAX_PATTERNS_PER_KEY_MAX: usize = 65536;

/// Maximum number of taps per morse
pub const MAX_TAPS_PER_MORSE: usize = 15;
