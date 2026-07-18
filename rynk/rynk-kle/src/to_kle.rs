//! Reverse conversion: an RMK/Rynk `[layout]` back into a Vial `layouts.keymap`
//! (KLE) — the inverse of `kle`/`layout`.
//!
//! The layout's canonical rendered layout is the decoded blob ([`rynk::layout::LayoutInfo`]):
//! each key's absolute center, size, and rotation. We emit each key on its own KLE
//! row placed absolutely, with the **rotation origin at the key's own center** —
//! reset the cursor to the center with `rx`/`ry`, step back to the unrotated
//! top-left with `x`/`y`, then tilt by `r`. KLE rotates the cap about `(rx, ry)`,
//! which is the center, so the result is a cap tilted about its own center: exactly
//! how RMK stores rotation. Encoders come back in Vial's convention: two 1u CW/CCW
//! switches side by side, centered on the knob.

use rynk::layout::{Encoder, Key, Rect, Variant};
use serde_json::{Map, Value, json};

fn round(v: f32) -> f64 {
    ((v as f64) * 1e4).round() / 1e4
}

/// One absolutely-placed KLE key. `r` is always emitted so it can't inherit the
/// previous key's angle; `rx`/`ry` snap the cursor to the center, `x`/`y` step to
/// the unrotated top-left.
fn kle_key(cx: f32, cy: f32, w: f32, h: f32, rect2: Option<&Rect>, r: f32, legend: String) -> Value {
    let mut o = Map::new();
    o.insert("r".into(), json!(round(r)));
    o.insert("rx".into(), json!(round(cx)));
    o.insert("ry".into(), json!(round(cy)));
    o.insert("x".into(), json!(round(-w / 2.0)));
    o.insert("y".into(), json!(round(-h / 2.0)));
    if (w - 1.0).abs() > 1e-4 {
        o.insert("w".into(), json!(round(w)));
    }
    if (h - 1.0).abs() > 1e-4 {
        o.insert("h".into(), json!(round(h)));
    }
    if let Some(rect2) = rect2 {
        o.insert("w2".into(), json!(round(rect2.w)));
        o.insert("h2".into(), json!(round(rect2.h)));
        o.insert("x2".into(), json!(round(rect2.x - cx + (w - rect2.w) / 2.0)));
        o.insert("y2".into(), json!(round(rect2.y - cy + (h - rect2.h) / 2.0)));
    }
    Value::Array(vec![Value::Object(o), Value::String(legend)])
}

fn key_row(k: &Key) -> Value {
    kle_key(
        k.rect.x,
        k.rect.y,
        k.rect.w,
        k.rect.h,
        k.rect2.as_ref(),
        k.r,
        format!("{},{}", k.row, k.col),
    )
}

fn encoder_rows(e: &Encoder) -> [Value; 2] {
    // Vial's knob convention: a 1u switch per rotary direction, CCW (`id,0`) and
    // CW (`id,1`) side by side — both must exist for Vial to offer both bindings.
    let legend = |dir: u8| format!("{},{dir}\n\n\n\n\n\n\n\n\ne", e.id);
    [
        kle_key(e.x - 0.5, e.y, 1.0, 1.0, None, 0.0, legend(0)),
        kle_key(e.x + 0.5, e.y, 1.0, 1.0, None, 0.0, legend(1)),
    ]
}

/// A KLE `layouts.keymap` array for one render variant.
pub(crate) fn variant_to_kle(v: &Variant) -> Value {
    let mut rows: Vec<Value> = v.keys.iter().map(key_row).collect();
    rows.extend(v.encoders.iter().flat_map(encoder_rows));
    Value::Array(rows)
}

/// Full reverse pipeline: a `keyboard.toml` → a minimal `vial.json` value. Builds
/// the layout blob with rmk-config, decodes it as the host does, and emits the
/// default render variant as KLE.
pub(crate) fn keyboard_toml_to_vial(text: &str) -> Result<Value, String> {
    let decoded = crate::decode_layout_document(text)?;
    let variant = decoded
        .info
        .variants
        .get(decoded.info.default_variant as usize)
        .ok_or("layout has no default variant")?;

    Ok(json!({
        "name": "Converted from RMK [layout]",
        "vendorId": "0x0000",
        "productId": "0x0000",
        "matrix": { "rows": decoded.rows, "cols": decoded.cols },
        "layouts": { "keymap": variant_to_kle(variant) }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_toml_to_vial_preserves_l_shaped_keys() {
        let vial =
            keyboard_toml_to_vial("[layout]\nrows = 1\ncols = 2\nmap = \"(0,0,@iso_enter) (0,1,@bae)\"").unwrap();
        let keymap = vial.pointer("/layouts/keymap").unwrap();
        let parsed = crate::kle::parse_keymap(keymap).unwrap();

        let iso = &parsed.keys[0];
        assert_eq!((iso.width, iso.height), (1.25, 2.0));
        assert_eq!((iso.width2, iso.height2, iso.x2, iso.y2), (1.5, 1.0, -0.25, 0.0));

        let bae = &parsed.keys[1];
        assert_eq!((bae.width, bae.height), (2.25, 1.0));
        assert_eq!((bae.width2, bae.height2, bae.x2, bae.y2), (1.5, 1.0, 0.75, -1.0));
    }
}
