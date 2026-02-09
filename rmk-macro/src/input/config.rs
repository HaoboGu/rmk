//! Configuration types for input device macros.

use syn::Path;

/// Input device publishing config.
pub struct InputDeviceConfig {
    pub event_type: Path,
}
