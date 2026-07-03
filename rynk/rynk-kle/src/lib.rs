//! Convert a physical keyboard layout between KLE/Vial JSON and RMK/Rynk's
//! `[layout]` — as a library, usable natively (the `rmkit layout` CLI) and on
//! the web (the `wasm` feature adds JS bindings).
//!
//! Forward ([`convert_kle`]): a raw [KLE](http://keyboard-layout-editor.com)
//! JSON export (an array of rows, optionally led by a metadata object) or a
//! Vial keyboard definition (`vial.json`, which wraps that same KLE blob in
//! `layouts.keymap`) becomes the `[layout]` section of a `keyboard.toml`. Both
//! carry the *physical* arrangement — key positions, widths, split gaps,
//! layout options — which is exactly what `[layout]` describes. Neither
//! carries keycodes, so no `[keymap]` is emitted.
//!
//! Reverse ([`to_kle::keyboard_toml_to_vial`]): a `keyboard.toml`'s `[layout]`
//! back into a minimal `vial.json`.
//!
//! Decode ([`decode_layout`]): any `[layout]` TOML into [`layout::LayoutInfo`]
//! via the real wire path — `rmk-config` builds the same compressed blob the
//! firmware serves over `GetLayout`, which is inflated and postcard-decoded
//! with the host types. What you get is exactly what a Rynk host sees.

pub mod kle;
pub mod to_kle;
pub mod to_layout;

/// The decoded host-side layout types (`LayoutInfo`, `Variant`, `Key`, …),
/// re-exported from `rynk` so consumers don't need it as a direct dependency.
pub use rynk::layout;

use serde_json::Value;

use crate::to_layout::{GenInput, Generated, generate};

/// Read `object.matrix.<key>` as a `u32`, defaulting to 0 when absent.
fn matrix_dim(root: &Value, key: &str) -> u32 {
    root.get("matrix")
        .and_then(|m| m.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32
}

/// Forward conversion on a parsed JSON input — a raw KLE export or a Vial
/// definition — into the `[layout]` section (plus shapes/variants) it encodes.
///
/// When the input has no VIA `row,col` legends (a plain KLE export), matrix
/// positions are assigned row-major from key placement and a warning is added
/// to [`Generated::warnings`].
pub fn convert_kle(root: &Value) -> Result<Generated, String> {
    // A raw KLE export ("Download JSON" on keyboard-layout-editor.com) is the bare
    // row array (kle-serial skips a leading metadata object); a Vial/VIA definition
    // wraps that array in `layouts.keymap` and adds `matrix` dims + option labels.
    let keymap = if root.is_array() {
        root
    } else {
        root.get("layouts").and_then(|l| l.get("keymap")).ok_or(
            "unrecognized input: expected a Vial definition (with `layouts.keymap`) \
             or a raw KLE JSON export (an array of rows)",
        )?
    };
    let labels = root.get("layouts").and_then(|l| l.get("labels"));

    let mut parsed = kle::parse_keymap(keymap)?;
    // A plain KLE export carries key labels, not VIA `row,col` legends — derive the
    // matrix from key placement instead.
    let fallback = !parsed.has_matrix_or_encoder();
    if fallback {
        kle::assign_matrix_by_position(&mut parsed);
    }
    let mut generated = generate(GenInput {
        keys: &parsed.keys,
        annotations: &parsed.annotations,
        matrix_rows: matrix_dim(root, "rows"),
        matrix_cols: matrix_dim(root, "cols"),
        labels,
    })?;
    if fallback {
        generated.warnings.insert(
            0,
            "no `row,col` legends found — matrix positions were assigned \
             row-major from key placement; adjust them to match your wiring"
                .to_string(),
        );
    }
    Ok(generated)
}

/// `[layout]` TOML text → decoded [`layout::LayoutInfo`], via the real blob
/// round-trip. The input is a full keyboard.toml, any TOML with a `[layout]`
/// section (e.g. [`Generated::inner_layout_toml`]), or a bare `rows`/`cols`/
/// `map` snippet.
pub fn decode_layout(text: &str) -> Result<layout::LayoutInfo, String> {
    let doc: toml::Value = toml::from_str(text).map_err(|e| format!("invalid TOML: {e}"))?;
    let layout = match doc.get("layout") {
        Some(l) => l,
        // A bare snippet (rows/cols/map at top level) is accepted as-is.
        None if doc.get("map").is_some() => &doc,
        None => return Err("no [layout] section (and no top-level `map`) in the input".to_string()),
    };
    let inner = toml::to_string(layout).map_err(|e| format!("re-serialize [layout]: {e}"))?;
    let blob = rmk_config::layout_blob_from_toml(&inner)?;
    if blob.is_empty() {
        return Err("the [layout] section has no `map` — nothing to render".to_string());
    }
    layout::LayoutInfo::from_compressed_blob(&blob).map_err(|e| format!("decode layout blob: {e}"))
}

/// JS bindings over the same pipeline, string-in / plain-object-out. Built
/// into a wasm package with `wasm-pack build --features wasm`.
#[cfg(feature = "wasm")]
mod wasm {
    use wasm_bindgen::prelude::*;

    fn js_err(e: impl std::fmt::Display) -> JsError {
        JsError::new(&e.to_string())
    }

    /// KLE export or vial.json text → `{ display_toml, inner_layout_toml, warnings }`.
    #[wasm_bindgen]
    pub fn convert_kle(json: &str) -> Result<JsValue, JsError> {
        let root: serde_json::Value = serde_json::from_str(json).map_err(|e| js_err(format!("invalid JSON: {e}")))?;
        let generated = crate::convert_kle(&root).map_err(js_err)?;
        serde_wasm_bindgen::to_value(&generated).map_err(js_err)
    }

    /// keyboard.toml text → a minimal vial.json, pretty-printed.
    #[wasm_bindgen]
    pub fn keyboard_toml_to_vial(toml_text: &str) -> Result<String, JsError> {
        let vial = crate::to_kle::keyboard_toml_to_vial(toml_text).map_err(js_err)?;
        serde_json::to_string_pretty(&vial).map_err(js_err)
    }

    /// `[layout]` TOML text → the decoded `LayoutInfo` as a plain JS object,
    /// for rendering a preview.
    #[wasm_bindgen]
    pub fn decode_layout(toml_text: &str) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(&crate::decode_layout(toml_text).map_err(js_err)?).map_err(js_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn convert_kle_accepts_vial_and_raw_kle() {
        // A vial.json-shaped input: `row,col` legends, matrix dims, a 2u cap.
        let vial = json!({"matrix": {"rows": 1, "cols": 2},
                          "layouts": {"keymap": [["0,0", {"w": 2.0}, "0,1"]]}});
        let g = convert_kle(&vial).unwrap();
        assert!(g.display_toml.contains("(0,1,@2u)"), "{}", g.display_toml);
        assert!(g.warnings.is_empty(), "{:?}", g.warnings);

        // A raw KLE export: label legends only — row-major fallback + warning.
        let kle = json!([{"name": "plain"}, ["Esc", "Q"], [{"w": 1.5}, "Tab"]]);
        let g = convert_kle(&kle).unwrap();
        assert!(g.display_toml.contains("(1,0,@1.5u)"), "{}", g.display_toml);
        assert!(g.warnings.iter().any(|w| w.contains("row-major")), "{:?}", g.warnings);
    }

    #[test]
    fn generated_layout_decodes_to_layout_info() {
        let vial = json!({"layouts": {"keymap": [["0,0", "0,1"]]}});
        let g = convert_kle(&vial).unwrap();
        let info = decode_layout(&g.inner_layout_toml).unwrap();
        assert_eq!(info.variants[0].keys.len(), 2);
    }

    #[test]
    fn decode_layout_accepts_full_keyboard_toml() {
        // Only [layout] is read — the rest of the config (chip, matrix, …) is
        // irrelevant to the rendered layout and must not be required.
        let info = decode_layout("[keyboard]\nname = \"x\"\n\n[layout]\nrows = 1\ncols = 1\nmap = \"(0,0)\"").unwrap();
        assert_eq!(info.variants.len(), 1);
        assert_eq!(info.variants[0].keys.len(), 1);
    }

    #[test]
    fn missing_layout_or_map_is_a_clear_error() {
        let e = decode_layout("[keyboard]\nname = \"x\"").unwrap_err();
        assert!(e.contains("no [layout] section"), "{e}");
        let e = decode_layout("[layout]\nrows = 1\ncols = 1").unwrap_err();
        assert!(e.contains("no `map`"), "{e}");
    }

    #[test]
    fn non_layout_json_gets_the_shape_hint() {
        let e = convert_kle(&json!({"a": 1})).unwrap_err();
        assert!(e.contains("layouts.keymap"), "{e}");
    }
}
