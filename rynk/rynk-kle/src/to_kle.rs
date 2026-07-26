//! Reverse conversion: an RMK/Rynk `[layout]` back into a Vial `layouts.keymap`
//! (KLE) — the inverse of `kle`/`layout`.
//!
//! The canonical rendered layout is [`rynk::layout::LayoutInfo`]: each key's
//! absolute center, size, rotation — and, for keys a `[r=deg@(px,py)]` region
//! placed, the authoring pivot. We emit each key on its own KLE row placed
//! absolutely, `r`/`rx`/`ry` always explicit so rows stay self-contained. A key
//! carrying its region exports the real cluster: `(r, rx, ry)` name the
//! authored pivot and the cap sits at its un-swung flat spot, so keys of one
//! region share one KLE rotation cluster. Without a region — or with a shape's
//! own residual angle, which KLE cannot stack on a shared pivot — the cap tilts
//! about its own center: `rx`/`ry` snap to the center, `x`/`y` step back to the
//! unrotated top-left. Encoders come back in Vial's convention: two 1u CW/CCW
//! switches side by side, centered on the knob.

use rynk::layout::{Encoder, Key, Region, Variant};
use serde_json::{Map, Value, json};

fn round(v: f32) -> f64 {
    ((v as f64) * 1e4).round() / 1e4
}

/// A residual own-center angle KLE can't attach to a shared-pivot key; matches
/// the converter's 1e-4 rounding grain.
const RESIDUAL_EPS: f32 = 1e-4;

/// Inverse of the build-time swing: where a rendered point sat in the flat frame.
fn unswing(x: f32, y: f32, reg: &Region) -> (f32, f32) {
    let (sin, cos) = reg.deg.to_radians().sin_cos();
    let (dx, dy) = (x - reg.px, y - reg.py);
    (reg.px + dx * cos + dy * sin, reg.py - dx * sin + dy * cos)
}

/// One absolutely-placed KLE key in `cluster`: rotate `deg` about `(px, py)`,
/// cap centered at flat `(fx, fy)`. `rect2` is the L-overhang's
/// `(w2, h2, dx2, dy2)` with the center offsets in the key frame.
fn kle_key(
    cluster: &Region,
    fx: f32,
    fy: f32,
    w: f32,
    h: f32,
    rect2: Option<(f32, f32, f32, f32)>,
    legend: String,
) -> Value {
    let mut o = Map::new();
    o.insert("r".into(), json!(round(cluster.deg)));
    o.insert("rx".into(), json!(round(cluster.px)));
    o.insert("ry".into(), json!(round(cluster.py)));
    o.insert("x".into(), json!(round(fx - w / 2.0 - cluster.px)));
    o.insert("y".into(), json!(round(fy - h / 2.0 - cluster.py)));
    if (w - 1.0).abs() > 1e-4 {
        o.insert("w".into(), json!(round(w)));
    }
    if (h - 1.0).abs() > 1e-4 {
        o.insert("h".into(), json!(round(h)));
    }
    if let Some((w2, h2, dx2, dy2)) = rect2 {
        o.insert("w2".into(), json!(round(w2)));
        o.insert("h2".into(), json!(round(h2)));
        o.insert("x2".into(), json!(round(dx2 + (w - w2) / 2.0)));
        o.insert("y2".into(), json!(round(dy2 + (h - h2) / 2.0)));
    }
    Value::Array(vec![Value::Object(o), Value::String(legend)])
}

fn key_row(k: &Key) -> Value {
    let legend = format!("{},{}", k.row, k.col);
    let rect2 = k
        .rect2
        .as_ref()
        .map(|r2| (r2.w, r2.h, r2.x - k.rect.x, r2.y - k.rect.y));
    match &k.pivot {
        // The real cluster: keys of one region share (r, rx, ry) and sit at
        // their un-swung flat spots.
        Some(reg) if (k.r - reg.deg).abs() <= RESIDUAL_EPS => {
            let (fx, fy) = unswing(k.rect.x, k.rect.y, reg);
            kle_key(reg, fx, fy, k.rect.w, k.rect.h, rect2, legend)
        }
        // No region, or a residual shape angle: a degenerate one-key cluster
        // tilting about the key's own center.
        _ => {
            let own = Region {
                deg: k.r,
                px: k.rect.x,
                py: k.rect.y,
            };
            kle_key(&own, k.rect.x, k.rect.y, k.rect.w, k.rect.h, rect2, legend)
        }
    }
}

fn encoder_rows(e: &Encoder) -> [Value; 2] {
    // Vial's knob convention: a 1u switch per rotary direction, CCW (`id,0`) and
    // CW (`id,1`) side by side — both must exist for Vial to offer both bindings.
    let legend = |dir: u8| format!("{},{dir}\n\n\n\n\n\n\n\n\ne", e.id);
    match &e.pivot {
        // The pair sits beside the knob in the flat frame and swings as a unit.
        Some(reg) => {
            let (fx, fy) = unswing(e.x, e.y, reg);
            [
                kle_key(reg, fx - 0.5, fy, 1.0, 1.0, None, legend(0)),
                kle_key(reg, fx + 0.5, fy, 1.0, 1.0, None, legend(1)),
            ]
        }
        None => {
            let own = |px| Region { deg: 0.0, px, py: e.y };
            [
                kle_key(&own(e.x - 0.5), e.x - 0.5, e.y, 1.0, 1.0, None, legend(0)),
                kle_key(&own(e.x + 0.5), e.x + 0.5, e.y, 1.0, 1.0, None, legend(1)),
            ]
        }
    }
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
pub fn keyboard_toml_to_vial(text: &str) -> Result<Value, String> {
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

    #[test]
    fn region_layout_exports_a_shared_cluster() {
        let vial =
            keyboard_toml_to_vial("[layout]\nrows = 1\ncols = 3\nmap = \"(0,0) [r=90@(2.5,0)] (0,1) (0,2)\"").unwrap();
        let parsed = crate::kle::parse_keymap(vial.pointer("/layouts/keymap").unwrap()).unwrap();
        let key = |r, c| parsed.keys.iter().find(|k| k.matrix == Some((r, c))).unwrap();
        // The flat key stays out of any cluster.
        assert_eq!(key(0, 0).rotation, 0.0);
        // The rotated pair shares the authored pivot, caps at their flat spots.
        for (col, flat_x) in [(1u8, 1.0f64), (2, 2.0)] {
            let k = key(0, col);
            assert_eq!((k.rotation, k.rx, k.ry), (90.0, 2.5, 0.0));
            assert!(
                (k.x - flat_x).abs() < 1e-4 && k.y.abs() < 1e-4,
                "(0,{col}) flat spot ({}, {})",
                k.x,
                k.y
            );
        }
    }

    #[test]
    fn residual_shape_r_falls_back_to_own_center() {
        // KLE can't stack "shared pivot + own residual angle": the key splits
        // out of the cluster and tilts about its own rendered center.
        let vial = keyboard_toml_to_vial(
            "[layout]\nrows = 1\ncols = 1\nmap = \"[r=15@(0,0)] (0,0,@tilt)\"\n[layout.shapes]\ntilt = { r = 10.0 }",
        )
        .unwrap();
        let parsed = crate::kle::parse_keymap(vial.pointer("/layouts/keymap").unwrap()).unwrap();
        let k = &parsed.keys[0];
        assert!((k.rotation - 25.0).abs() < 1e-4, "r = {}", k.rotation);
        // Own-center pivot = the rendered (swung) center of the flat (0.5, 0.5).
        let (sin, cos) = 15f64.to_radians().sin_cos();
        let (cx, cy) = (0.5 * cos - 0.5 * sin, 0.5 * sin + 0.5 * cos);
        assert!(
            (k.rx - cx).abs() < 5e-3 && (k.ry - cy).abs() < 5e-3,
            "pivot ({}, {}) vs center ({cx}, {cy})",
            k.rx,
            k.ry
        );
    }

    #[test]
    fn rotated_encoder_exports_its_cluster() {
        let vial = keyboard_toml_to_vial("[layout]\nrows = 1\ncols = 1\nmap = \"[r=30@(0,0)] (0,0) (e,0)\"").unwrap();
        let parsed = crate::kle::parse_keymap(vial.pointer("/layouts/keymap").unwrap()).unwrap();
        let pair: Vec<_> = parsed.keys.iter().filter(|k| k.encoder.is_some()).collect();
        assert_eq!(pair.len(), 2);
        for k in &pair {
            assert_eq!((k.rotation, k.rx, k.ry), (30.0, 0.0, 0.0), "switch shares the cluster");
        }
        // Feeding the export back keeps one uniform cluster per knob.
        let g = crate::convert_kle(&vial).unwrap();
        assert!(
            !g.warnings.iter().any(|w| w.contains("mixes rotation clusters")),
            "{:?}",
            g.warnings
        );
    }
}
