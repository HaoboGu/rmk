//! Configuration types for controller macros.

use proc_macro2::TokenStream;
use syn::Path;

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
