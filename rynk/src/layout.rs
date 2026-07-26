//! Host-decoded physical key layout.
//!
//! `GetLayout` streams an opaque, compressed blob the firmware never decodes.
//! [`Client::get_layout`](crate::Client::get_layout) reassembles the pages and
//! decodes them with [`LayoutInfo::from_compressed_blob`].
//!
//! The types live in `rmk-config` — the crate that builds the blob — so
//! producer and decoder share one definition and can't drift.

pub use rmk_config::layout::{Encoder, Key, LayoutInfo, Rect, Region, Variant};
