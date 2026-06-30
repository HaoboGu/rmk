//! Host-decoded physical key layout.
//!
//! `GetLayout` streams an opaque, compressed blob the firmware never decodes.
//! [`Client::get_layout`](crate::Client::get_layout) reassembles the pages,
//! inflates them, and postcard-decodes the result into [`LayoutInfo`].
//!
//! These types mirror the build-time producer in `rmk-config`'s `layout.rs`
//! field-for-field — postcard is positional, so the order must match exactly.
//! The cross-crate match is by hand because `rmk-config` is a build-dependency
//! of `rmk-types` (no back-edge).

/// A key's outline rectangle in key-units.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// One key's placement: matrix position, center, size, rotation, and an
/// optional second rectangle for L-shaped keys (ISO/big-ass Enter).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Key {
    pub row: u8,
    pub col: u8,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub rect2: Option<Rect>,
}

/// One encoder's placement. Encoders are variant-invariant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Encoder {
    pub id: u8,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
}

/// One render variant (e.g. ANSI / ISO) over the shared keymap.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Variant {
    pub name: String,
    pub keys: Vec<Key>,
}

/// The decoded physical layout: render variants plus shared encoders.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LayoutInfo {
    pub default_variant: u8,
    pub variants: Vec<Variant>,
    pub encoders: Vec<Encoder>,
}
