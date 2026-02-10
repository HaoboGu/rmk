//! Configuration types for event system macros.
//!
//! Merged from input/config.rs and controller/config.rs.

use proc_macro2::TokenStream;
use syn::Path;

/// Input device publishing config.
pub struct InputDeviceConfig {
    pub event_type: Path,
}

/// Input processor subscription config.
pub struct InputProcessorConfig {
    pub event_types: Vec<Path>,
}

/// Controller subscription config.
pub struct ControllerConfig {
    pub event_types: Vec<Path>,
    pub poll_interval_ms: Option<u64>,
}

/// Controller event channel config (channel_size, subs, pubs).
pub struct ControllerEventChannelConfig {
    pub channel_size: Option<TokenStream>,
    pub subs: Option<TokenStream>,
    pub pubs: Option<TokenStream>,
}
