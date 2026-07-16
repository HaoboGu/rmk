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
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// One key's placement: matrix position, outline `rect` (center + size),
/// rotation, and an optional second rectangle for L-shaped keys (ISO/big-ass
/// Enter). `r` rotates the whole key, `rect2` included.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Key {
    pub row: u8,
    pub col: u8,
    pub rect: Rect,
    pub r: f32,
    pub rect2: Option<Rect>,
}

/// One encoder's placement within a variant: a fixed 1u knob, so just its
/// center — never resized, rotated, or L-shaped.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Encoder {
    pub id: u8,
    pub x: f32,
    pub y: f32,
}

/// One render variant (e.g. ANSI / ISO): its own keys and encoders. A hidden key
/// reflows the tokens after it — encoders included — so each variant carries its
/// own encoder positions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Variant {
    pub name: String,
    pub keys: Vec<Key>,
    pub encoders: Vec<Encoder>,
}

/// The decoded physical layout: one entry per render variant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LayoutInfo {
    pub default_variant: u8,
    pub variants: Vec<Variant>,
}

impl LayoutInfo {
    /// An empty layout, emitted when firmware was built without a `[layout].map`.
    pub fn empty() -> Self {
        Self {
            default_variant: 0,
            variants: Vec::new(),
        }
    }

    /// Decode the compressed `GetLayout` payload produced by `rmk-config`.
    ///
    /// The blob is raw DEFLATE containing postcard-encoded [`LayoutInfo`]. An
    /// empty blob is a valid "empty" layout.
    pub fn from_compressed_blob(blob: &[u8]) -> Result<Self, String> {
        if blob.is_empty() {
            return Ok(Self::empty());
        }

        let inflated = miniz_oxide::inflate::decompress_to_vec(blob).map_err(|e| format!("inflate failed: {e}"))?;
        postcard::from_bytes(&inflated).map_err(|e| format!("decode failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, LayoutInfo, Rect, Variant};

    #[test]
    fn empty_blob_decodes_to_empty_layout() {
        assert_eq!(LayoutInfo::from_compressed_blob(&[]).unwrap(), LayoutInfo::empty());
    }

    #[test]
    fn compressed_blob_round_trips() {
        let layout = LayoutInfo {
            default_variant: 0,
            variants: vec![Variant {
                name: "default".into(),
                keys: vec![Key {
                    row: 0,
                    col: 1,
                    rect: Rect {
                        x: 1.5,
                        y: 0.5,
                        w: 1.0,
                        h: 1.0,
                    },
                    r: 0.0,
                    rect2: None,
                }],
                encoders: Vec::new(),
            }],
        };
        let raw = postcard::to_allocvec(&layout).unwrap();
        let blob = miniz_oxide::deflate::compress_to_vec(&raw, 6);
        assert_eq!(LayoutInfo::from_compressed_blob(&blob).unwrap(), layout);
    }
}
