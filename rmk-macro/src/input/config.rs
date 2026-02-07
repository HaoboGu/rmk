//! Configuration types for input device and processor macros.

use syn::Path;

/// Input device publishing config.
pub struct InputDeviceConfig {
    pub event_type: Path,
}

/// Input processor subscription config.
pub struct InputProcessorConfig {
    pub event_types: Vec<Path>,
}
