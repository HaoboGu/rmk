//! Build-time physical key layout.
//!
//! Walk a cursor over the `[layout].map` tokens (keys, encoders, gaps, `[y=]`
//! steps), apply each `[[layout.variant]]` overlay, and produce a compressed,
//! opaque blob the firmware streams verbatim over `GetLayout` (it never decodes
//! it). The host inflates + postcard-decodes the blob into `LayoutInfo`.
//!
//! The `#[derive(Serialize)]` mirror types here MUST match the host-decode types
//! in the `rynk` host crate (`rynk::layout::*`) field-for-field — a round-trip
//! test guards the layout, but the cross-crate match is by hand because
//! `rmk-config` is a build-dependency of `rmk-types` (no back-edge).
//!
//! See `rynk-layout-geometry.md` for the model.

use std::collections::{HashMap, HashSet};

use pest::Parser;
use serde::{Deserialize, Serialize};

use crate::LayoutTomlConfig;
use crate::keymap::{ConfigParser, Rule};

// ── Wire mirror types (must match rmk-types `protocol::rynk::*`) ─────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Key {
    row: u8,
    col: u8,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    rect2: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Encoder {
    id: u8,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Variant {
    name: String,
    keys: Vec<Key>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct LayoutInfo {
    default_variant: u8,
    variants: Vec<Variant>,
    encoders: Vec<Encoder>,
}

// ── Shapes ──────────────────────────────────────────────────────────────────

/// A resolved shape: every default applied. `rect2` is `(w2, h2, x2, y2)`.
#[derive(Clone, Copy, Debug)]
struct Shape {
    w: f32,
    h: f32,
    x: f32,
    y: f32,
    r: f32,
    rect2: Option<(f32, f32, f32, f32)>,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            w: 1.0,
            h: 1.0,
            x: 0.0,
            y: 0.0,
            r: 0.0,
            rect2: None,
        }
    }
}

impl From<&crate::ShapeToml> for Shape {
    fn from(t: &crate::ShapeToml) -> Self {
        let rect2 =
            t.w2.map(|w2| (w2, t.h2.unwrap_or(1.0), t.x2.unwrap_or(0.0), t.y2.unwrap_or(0.0)));
        Shape {
            w: t.w.unwrap_or(1.0),
            h: t.h.unwrap_or(1.0),
            x: t.x.unwrap_or(0.0),
            y: t.y.unwrap_or(0.0),
            r: t.r.unwrap_or(0.0),
            rect2,
        }
    }
}

/// The shipped stock shapes (`@2u`, `@1.5u`, …, `@iso_enter`, …). Keyed without
/// the leading `@`. User `[layout.shapes]` entries of the same name override.
fn stock_shapes() -> HashMap<String, Shape> {
    let mut m = HashMap::new();
    // Width family `@Nu` — N units wide, 1u tall.
    for (name, w) in [
        ("1.25u", 1.25),
        ("1.5u", 1.5),
        ("1.75u", 1.75),
        ("2u", 2.0),
        ("2.25u", 2.25),
        ("2.75u", 2.75),
        ("3u", 3.0),
        ("6.25u", 6.25),
        ("7u", 7.0),
    ] {
        m.insert(name.to_string(), Shape { w, ..Shape::default() });
    }
    // Tall: 1u wide, 2u tall (numpad + / Enter).
    m.insert(
        "2uv".to_string(),
        Shape {
            h: 2.0,
            ..Shape::default()
        },
    );
    // Stepped Caps: a single 1.75u rect (the step is a 3-D detail).
    m.insert(
        "stepped_caps".to_string(),
        Shape {
            w: 1.75,
            ..Shape::default()
        },
    );
    // ISO Enter: a 1.25×2 bar + a 1.5×1 top overhang (true L, two rects).
    m.insert(
        "iso_enter".to_string(),
        Shape {
            w: 1.25,
            h: 2.0,
            y: -1.0,
            rect2: Some((1.5, 1.0, -0.25, 0.0)),
            ..Shape::default()
        },
    );
    // Big-ass Enter: 2.25×1 bottom + 1.5×1 top one row up, right-aligned.
    m.insert(
        "bae".to_string(),
        Shape {
            w: 2.25,
            h: 1.0,
            rect2: Some((1.5, 1.0, 0.375, -1.0)),
            ..Shape::default()
        },
    );
    m
}

// ── Token stream ──────────────────────────────────────────────────────────────

enum Token {
    Key {
        row: u8,
        col: u8,
        shape: Option<String>,
    },
    Encoder {
        id: u8,
        shape: Option<String>,
    },
    /// A `[n]` horizontal gap, in key-units.
    Gap(f32),
    /// A `[y=n]` extra vertical step for the next row.
    VStep(f32),
    Newline,
}

fn parse_u8(s: &str, what: &str) -> Result<u8, String> {
    s.parse::<u8>()
        .map_err(|e| format!("keyboard.toml: bad {what} '{s}' in layout.map: {e}"))
}

fn parse_f32(s: &str, what: &str) -> Result<f32, String> {
    s.parse::<f32>()
        .map_err(|e| format!("keyboard.toml: bad {what} '{s}' in layout.map: {e}"))
}

/// The `@`-stripped name inside a `shape_ref` pair.
fn shape_name_of(pair: pest::iterators::Pair<Rule>) -> String {
    pair.into_inner()
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default()
}

fn parse_tokens(map: &str) -> Result<Vec<Token>, String> {
    let pairs =
        ConfigParser::parse(Rule::layout_map, map).map_err(|e| format!("keyboard.toml: Error in `layout.map`: {e}"))?;
    let mut tokens = Vec::new();
    for pair in pairs {
        if pair.as_rule() != Rule::layout_map {
            continue;
        }
        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::keypos_info => {
                    let mut it = inner.into_inner();
                    let row = parse_u8(it.next().ok_or("missing row")?.as_str(), "row")?;
                    let col = parse_u8(it.next().ok_or("missing col")?.as_str(), "col")?;
                    let mut shape = None;
                    for part in it {
                        if part.as_rule() == Rule::shape_ref {
                            shape = Some(shape_name_of(part));
                        }
                    }
                    tokens.push(Token::Key { row, col, shape });
                }
                Rule::encoder_info => {
                    let mut it = inner.into_inner();
                    let id = parse_u8(it.next().ok_or("missing encoder id")?.as_str(), "encoder id")?;
                    let mut shape = None;
                    for part in it {
                        if part.as_rule() == Rule::shape_ref {
                            shape = Some(shape_name_of(part));
                        }
                    }
                    tokens.push(Token::Encoder { id, shape });
                }
                Rule::spacer => {
                    let u = inner.into_inner().next().ok_or("missing gap")?.as_str();
                    tokens.push(Token::Gap(parse_f32(u, "gap")?));
                }
                Rule::vertical => {
                    let u = inner.into_inner().next().ok_or("missing y-step")?.as_str();
                    tokens.push(Token::VStep(parse_f32(u, "y-step")?));
                }
                Rule::newline => tokens.push(Token::Newline),
                _ => {}
            }
        }
    }
    Ok(tokens)
}

// ── The cursor walk ───────────────────────────────────────────────────────────

/// Cursor state. A row's baseline `y` is the TOP of the row; a key stores its
/// center. The advance to the next row is *lazy*: a newline only arms a break
/// (so a lone `[y=n]` line doesn't itself consume a row), and the next key /
/// encoder / gap performs the `1 + pending_vstep` drop.
struct Walker {
    cursor_x: f32,
    baseline_y: f32,
    row_has_content: bool,
    break_pending: bool,
    pending_vstep: f32,
}

impl Walker {
    fn new() -> Self {
        Walker {
            cursor_x: 0.0,
            baseline_y: 0.0,
            row_has_content: false,
            break_pending: false,
            pending_vstep: 0.0,
        }
    }

    fn advance_if_pending(&mut self) {
        if self.break_pending {
            self.baseline_y += 1.0 + self.pending_vstep;
            self.pending_vstep = 0.0;
            self.cursor_x = 0.0;
            self.break_pending = false;
            self.row_has_content = false;
        }
    }
}

fn resolve_shape(name: Option<&str>, shapes: &HashMap<String, Shape>) -> Result<Shape, String> {
    match name {
        None => Ok(Shape::default()),
        Some(n) => shapes
            .get(n)
            .copied()
            .ok_or_else(|| format!("keyboard.toml: unknown shape '@{n}' in layout.map")),
    }
}

/// Walk one variant: `overrides` reshape a key, `hidden` drop it from the walk
/// (so following keys reflow). Returns this variant's keys and encoders.
fn walk(
    tokens: &[Token],
    shapes: &HashMap<String, Shape>,
    overrides: &HashMap<(u8, u8), String>,
    hidden: &HashSet<(u8, u8)>,
) -> Result<(Vec<Key>, Vec<Encoder>), String> {
    let mut w = Walker::new();
    let mut keys = Vec::new();
    let mut encoders = Vec::new();
    for tok in tokens {
        match tok {
            Token::Newline => {
                if w.row_has_content {
                    w.break_pending = true;
                }
            }
            Token::VStep(n) => {
                // `[y=n]` adjusts the gap above the NEXT row break. A marker with no
                // row to attach to (e.g. before the first key) has nothing to adjust,
                // so drop it rather than leak it into a later break.
                if w.row_has_content {
                    w.pending_vstep += n;
                }
            }
            Token::Gap(g) => {
                w.advance_if_pending();
                w.cursor_x += g;
            }
            Token::Key { row, col, shape } => {
                w.advance_if_pending();
                // Hidden: removed from the walk — advances nothing, so following keys reflow.
                if hidden.contains(&(*row, *col)) {
                    continue;
                }
                let name = overrides.get(&(*row, *col)).map(String::as_str).or(shape.as_deref());
                let s = resolve_shape(name, shapes)?;
                let cx = w.cursor_x + s.w / 2.0 + s.x;
                let cy = w.baseline_y + s.h / 2.0 + s.y;
                let rect2 = s.rect2.map(|(w2, h2, x2, y2)| Rect {
                    x: cx + x2,
                    y: cy + y2,
                    w: w2,
                    h: h2,
                });
                keys.push(Key {
                    row: *row,
                    col: *col,
                    x: cx,
                    y: cy,
                    w: s.w,
                    h: s.h,
                    r: s.r,
                    rect2,
                });
                w.cursor_x += s.w;
                w.row_has_content = true;
            }
            Token::Encoder { id, shape } => {
                w.advance_if_pending();
                let s = resolve_shape(shape.as_deref(), shapes)?;
                let cx = w.cursor_x + s.w / 2.0 + s.x;
                let cy = w.baseline_y + s.h / 2.0 + s.y;
                encoders.push(Encoder {
                    id: *id,
                    x: cx,
                    y: cy,
                    w: s.w,
                    h: s.h,
                    r: s.r,
                });
                w.cursor_x += s.w;
                w.row_has_content = true;
            }
        }
    }
    Ok((keys, encoders))
}

/// Parse a quoted `"(r,c)"` overlay key into `(row, col)`.
fn parse_rc(s: &str) -> Result<(u8, u8), String> {
    let inner = s.trim().trim_start_matches('(').trim_end_matches(')');
    let mut it = inner.split(',');
    let r = parse_u8(it.next().unwrap_or("").trim(), "variant target row")?;
    let c = parse_u8(it.next().unwrap_or("").trim(), "variant target col")?;
    Ok((r, c))
}

fn parse_overrides(shapes: &Option<HashMap<String, String>>) -> Result<HashMap<(u8, u8), String>, String> {
    let mut out = HashMap::new();
    if let Some(map) = shapes {
        for (rc, name) in map {
            out.insert(parse_rc(rc)?, name.trim_start_matches('@').to_string());
        }
    }
    Ok(out)
}

fn parse_hidden(hidden: &Option<Vec<String>>) -> Result<HashSet<(u8, u8)>, String> {
    let mut out = HashSet::new();
    if let Some(list) = hidden {
        for rc in list {
            out.insert(parse_rc(rc)?);
        }
    }
    Ok(out)
}

/// Every f32 dimension of a shape is finite (rejects `nan`/`inf` from TOML).
fn shape_is_finite(s: &Shape) -> bool {
    [s.w, s.h, s.x, s.y, s.r].iter().all(|v| v.is_finite())
        && s.rect2
            .map_or(true, |(w2, h2, x2, y2)| [w2, h2, x2, y2].iter().all(|v| v.is_finite()))
}

/// `expected_encoders` is the board's physical encoder count (`Some` from the
/// real build, `None` from the standalone TOML helper which has no board).
fn build_layout_info(
    layout: &LayoutTomlConfig,
    expected_encoders: Option<usize>,
) -> Result<Option<LayoutInfo>, String> {
    let Some(map) = &layout.map else {
        return Ok(None);
    };
    let tokens = parse_tokens(map)?;

    // Collect the real key coordinates (bounds-checking each), so variant
    // overlays can be validated against them — `layout_blob_from_toml` skips the
    // resolver's own bounds check, so guard here too.
    let mut key_coords: HashSet<(u8, u8)> = HashSet::new();
    for tok in &tokens {
        if let Token::Key { row, col, .. } = tok {
            if *row >= layout.rows || *col >= layout.cols {
                return Err(format!(
                    "keyboard.toml: layout.map coordinate ({row},{col}) is out of bounds ([0..{}], [0..{}])",
                    layout.rows.saturating_sub(1),
                    layout.cols.saturating_sub(1)
                ));
            }
            key_coords.insert((*row, *col));
        }
    }

    let mut shapes = stock_shapes();
    if let Some(user) = &layout.shapes {
        for (k, v) in user {
            let s = Shape::from(v);
            if !shape_is_finite(&s) {
                return Err(format!(
                    "keyboard.toml: shape '{k}' has a non-finite (nan/inf) dimension"
                ));
            }
            shapes.insert(k.clone(), s);
        }
    }

    let no_variants = Vec::new();
    let variants_toml = layout.variant.as_ref().unwrap_or(&no_variants);
    // `default_variant` is serialized as a u8 index, so at most 256 variants.
    if variants_toml.len() > u8::MAX as usize + 1 {
        return Err(format!(
            "keyboard.toml: too many [[layout.variant]] ({}); at most {}",
            variants_toml.len(),
            u8::MAX as usize + 1
        ));
    }
    // A variant `shapes`/`hidden` target that names no real key is a silent
    // no-op — reject it, mirroring the unknown-shape error.
    for v in variants_toml {
        let targets = v
            .shapes
            .iter()
            .flatten()
            .map(|(k, _)| k)
            .chain(v.hidden.iter().flatten());
        for rc in targets {
            let coord = parse_rc(rc)?;
            if !key_coords.contains(&coord) {
                return Err(format!(
                    "keyboard.toml: variant '{}' targets ({},{}) which is not a key in layout.map",
                    v.name, coord.0, coord.1
                ));
            }
        }
    }

    // The base walk (no overlay) gives the canonical keys AND the variant-
    // invariant encoders — a hidden key before an encoder must not shift it.
    let (base_keys, encoders) = walk(&tokens, &shapes, &HashMap::new(), &HashSet::new())?;

    let mut walked: Vec<(String, Vec<Key>)> = Vec::new();
    if variants_toml.is_empty() {
        walked.push(("default".to_string(), base_keys));
    } else {
        for v in variants_toml {
            let overrides = parse_overrides(&v.shapes)?;
            let hidden = parse_hidden(&v.hidden)?;
            let (keys, _) = walk(&tokens, &shapes, &overrides, &hidden)?;
            walked.push((v.name.clone(), keys));
        }
    }

    // Resolve by name to a 0-based index; an absent or unknown name falls back
    // to variant 0 (per design decision #6).
    let default_variant = layout
        .default_variant
        .as_ref()
        .and_then(|name| walked.iter().position(|(n, _)| n == name))
        .unwrap_or(0) as u8;

    // Encoder ids must be unique and dense (0..N); and if any encoder geometry is
    // given at all, it must cover every physical encoder the board declares.
    let mut ids: Vec<u8> = encoders.iter().map(|e| e.id).collect();
    ids.sort_unstable();
    for (expected, &id) in ids.iter().enumerate() {
        if id as usize != expected {
            return Err(format!(
                "keyboard.toml: encoder ids in layout.map must be unique and cover 0..{} (got {ids:?})",
                ids.len()
            ));
        }
    }
    if let Some(n) = expected_encoders {
        if !encoders.is_empty() && encoders.len() != n {
            return Err(format!(
                "keyboard.toml: layout.map has {} encoder (e,id) tokens but the board declares {n}",
                encoders.len()
            ));
        }
    }

    let variants = walked.into_iter().map(|(name, keys)| Variant { name, keys }).collect();
    Ok(Some(LayoutInfo {
        default_variant,
        variants,
        encoders,
    }))
}

/// Build the compressed layout blob from a `[layout]`-section TOML string.
///
/// Exposed for cross-crate end-to-end tests (the host crate decodes the result),
/// so the producer here and the host-decode types can't drift unnoticed.
pub fn layout_blob_from_toml(layout_toml: &str) -> Result<Vec<u8>, String> {
    let layout: LayoutTomlConfig = toml::from_str(layout_toml).map_err(|e| e.to_string())?;
    build_layout_blob(&layout, None)
}

/// Build the compressed, opaque layout blob (empty when there's no `map`).
/// `expected_encoders` is the board's physical encoder count, or `None` to skip
/// that cross-check (the standalone TOML helper has no board).
pub(crate) fn build_layout_blob(
    layout: &LayoutTomlConfig,
    expected_encoders: Option<usize>,
) -> Result<Vec<u8>, String> {
    let Some(info) = build_layout_info(layout, expected_encoders)? else {
        return Ok(Vec::new());
    };
    let bytes =
        postcard::to_allocvec(&info).map_err(|e| format!("keyboard.toml: layout geometry serialize failed: {e}"))?;
    // Raw DEFLATE at max level — compression runs at build time, so the extra
    // effort is free, and the host inflates with the matching `miniz_oxide` decoder.
    Ok(miniz_oxide::deflate::compress_to_vec(&bytes, 10))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn info_of(toml: &str) -> LayoutInfo {
        let cfg: LayoutTomlConfig = toml::from_str(toml).unwrap();
        build_layout_info(&cfg, None).unwrap().unwrap()
    }

    fn key<'a>(v: &'a Variant, row: u8, col: u8) -> &'a Key {
        v.keys
            .iter()
            .find(|k| k.row == row && k.col == col)
            .expect("key present")
    }

    #[test]
    fn bare_keys_make_a_unit_grid() {
        let info = info_of("rows = 1\ncols = 3\nlayers = 1\nmap = \"(0,0) (0,1) (0,2)\"");
        let v = &info.variants[0];
        assert_eq!(v.name, "default");
        for (i, k) in v.keys.iter().enumerate() {
            assert!(approx(k.x, i as f32 + 0.5), "center x");
            assert!(approx(k.y, 0.5), "center y");
            assert!(approx(k.w, 1.0) && approx(k.h, 1.0));
            assert!(k.rect2.is_none());
        }
    }

    #[test]
    fn stock_width_moves_the_cursor() {
        // A 2u key then a 1u key: 2u is centered at 1.0, next at 2.5.
        let info = info_of("rows = 1\ncols = 2\nlayers = 1\nmap = \"(0,0,@2u) (0,1)\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 0, 0).x, 1.0) && approx(key(v, 0, 0).w, 2.0));
        assert!(approx(key(v, 0, 1).x, 2.5));
    }

    #[test]
    fn y_step_is_one_shot_and_lazy() {
        // Row 1 lands 1 + 0.25 = 1.25 below row 0 despite the marker on its own line.
        let info = info_of("rows = 2\ncols = 2\nlayers = 1\nmap = \"\"\"\n(0,0) (0,1)\n[y=0.25]\n(1,0) (1,1)\n\"\"\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 0, 0).y, 0.5));
        assert!(approx(key(v, 1, 0).y, 1.75)); // 0.5 + 1.25
    }

    #[test]
    fn leading_y_step_is_dropped() {
        // A `[y=n]` before the first row has no preceding row to push off — it
        // must NOT leak into the row 0 → row 1 break and shift the whole board.
        let info = info_of("rows = 3\ncols = 1\nlayers = 1\nmap = \"\"\"\n[y=0.5]\n(0,0)\n(1,0)\n(2,0)\n\"\"\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 0, 0).y, 0.5));
        assert!(approx(key(v, 1, 0).y, 1.5)); // not 2.0
        assert!(approx(key(v, 2, 0).y, 2.5)); // not 3.0
    }

    #[test]
    fn unknown_default_variant_falls_back_to_zero() {
        // Per decision #6: an unknown (or absent) default_variant resolves to 0,
        // it does not fail the build.
        let info = info_of(
            "rows = 1\ncols = 1\nlayers = 1\ndefault_variant = \"typo\"\nmap = \"(0,0)\"\n[[variant]]\nname = \"a\"\n[[variant]]\nname = \"b\"",
        );
        assert_eq!(info.default_variant, 0);
        // A board with no [[variant]] at all + a named default still resolves to 0.
        let info2 = info_of("rows = 1\ncols = 1\nlayers = 1\ndefault_variant = \"x\"\nmap = \"(0,0)\"");
        assert_eq!(info2.default_variant, 0);
    }

    #[test]
    fn encoder_ids_must_be_unique_and_dense() {
        let ok = info_of("rows = 1\ncols = 2\nlayers = 1\nmap = \"(0,0) (e,0) (0,1) (e,1)\"");
        assert_eq!(ok.encoders.len(), 2);
        let dup: LayoutTomlConfig =
            toml::from_str("rows = 1\ncols = 1\nlayers = 1\nmap = \"(0,0) (e,0) (e,0)\"").unwrap();
        assert!(build_layout_info(&dup, None).is_err(), "duplicate encoder id must fail");
        let gap: LayoutTomlConfig =
            toml::from_str("rows = 1\ncols = 1\nlayers = 1\nmap = \"(0,0) (e,0) (e,2)\"").unwrap();
        assert!(
            build_layout_info(&gap, None).is_err(),
            "non-dense encoder ids must fail"
        );
    }

    #[test]
    fn out_of_bounds_coord_is_rejected() {
        let cfg: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nlayers = 1\nmap = \"(0,0) (0,5)\"").unwrap();
        assert!(build_layout_info(&cfg, None).is_err());
    }

    #[test]
    fn non_finite_shape_is_rejected() {
        let nan: LayoutTomlConfig =
            toml::from_str("rows = 1\ncols = 1\nlayers = 1\nmap = \"(0,0,@bad)\"\n[shapes]\nbad = { w = nan }")
                .unwrap();
        assert!(build_layout_info(&nan, None).is_err(), "nan width must fail");
        let inf: LayoutTomlConfig =
            toml::from_str("rows = 1\ncols = 1\nlayers = 1\nmap = \"(0,0,@big)\"\n[shapes]\nbig = { x = inf }")
                .unwrap();
        assert!(build_layout_info(&inf, None).is_err(), "inf nudge must fail");
    }

    #[test]
    fn variant_target_must_be_a_real_key() {
        // `hidden` names (0,9), which is not a key in the 1x2 map.
        let cfg: LayoutTomlConfig = toml::from_str(
            "rows = 1\ncols = 2\nlayers = 1\nmap = \"(0,0) (0,1)\"\n[[variant]]\nname = \"a\"\nhidden = [\"(0,9)\"]",
        )
        .unwrap();
        assert!(build_layout_info(&cfg, None).is_err());
    }

    #[test]
    fn encoders_are_variant_invariant_base_positions() {
        // The `mini` variant hides (0,0) and reflows its KEYS left, but the flat
        // encoder list is taken from the base walk, so the knob stays at x=2.5.
        let info = info_of(
            "rows = 1\ncols = 2\nlayers = 1\nmap = \"(0,0) (0,1) (e,0)\"\n[[variant]]\nname = \"full\"\n[[variant]]\nname = \"mini\"\nhidden = [\"(0,0)\"]",
        );
        assert_eq!(info.encoders.len(), 1);
        assert!(
            approx(info.encoders[0].x, 2.5),
            "encoder x = {} (must be base 2.5)",
            info.encoders[0].x
        );
        let mini = info.variants.iter().find(|v| v.name == "mini").unwrap();
        assert!(
            mini.keys.iter().all(|k| !(k.row == 0 && k.col == 0)),
            "(0,0) is hidden in mini"
        );
    }

    #[test]
    fn encoder_count_must_match_board() {
        // 1 `(e,id)` token but the board declares 2 encoders → error.
        let one: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nlayers = 1\nmap = \"(0,0) (e,0)\"").unwrap();
        assert!(
            build_layout_blob(&one, Some(2)).is_err(),
            "1 token vs 2 board encoders must fail"
        );
        assert!(build_layout_blob(&one, Some(1)).is_ok(), "matching count is fine");
        // Providing NO encoder geometry on a board that has encoders is allowed.
        let none: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nlayers = 1\nmap = \"(0,0)\"").unwrap();
        assert!(
            build_layout_blob(&none, Some(3)).is_ok(),
            "opting out of encoder geometry is allowed"
        );
    }

    const CORNE_SPLIT: &str = r#"
rows = 4
cols = 12
layers = 1
default_variant = "corne42"
map = """
(0,0,L,@cP) (0,1,L,@cR) (0,2,L,@cM) (0,3,L,@cI) (0,4,L,@cI) (0,5,L,@cX) [1.0] (0,6,R,@cX) (0,7,R,@cI) (0,8,R,@cI) (0,9,R,@cM) (0,10,R,@cR) (0,11,R,@cP)
(1,0,L,@cP) (1,1,L,@cR) (1,2,L,@cM) (1,3,L,@cI) (1,4,L,@cI) (1,5,L,@cX) [1.0] (1,6,R,@cX) (1,7,R,@cI) (1,8,R,@cI) (1,9,R,@cM) (1,10,R,@cR) (1,11,R,@cP)
(2,0,L,@cP) (2,1,L,@cR) (2,2,L,@cM) (2,3,L,@cI) (2,4,L,@cI) (2,5,L,@cX) [1.0] (2,6,R,@cX) (2,7,R,@cI) (2,8,R,@cI) (2,9,R,@cM) (2,10,R,@cR) (2,11,R,@cP)
[y=0.05]
[3.5] (3,3,L,@thumbL) (3,4,L) (3,5,L,@thumbR) [1.0] (3,6,R,@thumbL) (3,7,R) (3,8,R,@thumbR)
"""

[shapes]
cP = { y = 0.55 }
cR = { y = 0.25 }
cM = { y = 0.0 }
cI = { y = 0.10 }
cX = { y = 0.25 }
thumbL = { r = 15.0 }
thumbR = { r = -15.0 }

[[variant]]
name = "corne42"

[[variant]]
name = "corne36"
hidden = ["(0,0)", "(1,0)", "(2,0)", "(0,11)", "(1,11)", "(2,11)"]
"#;

    #[test]
    fn y_step_shifts_every_row_below() {
        // `[y=1]` between row 0 and row 1 pushes the gap by +1; because the
        // baseline accumulates, EVERY row below row 0 is shifted down by 1.
        let info = info_of("rows = 3\ncols = 1\nlayers = 1\nmap = \"\"\"\n(0,0)\n[y=1]\n(1,0)\n(2,0)\n\"\"\"");
        let v = &info.variants[0];
        // Stored y is the key CENTER (row top + 0.5). row-tops: 0, 2, 3.
        assert!(approx(key(v, 0, 0).y, 0.5)); // top 0
        assert!(approx(key(v, 1, 0).y, 2.5)); // top 2  (= 1 + 1 shift)
        assert!(approx(key(v, 2, 0).y, 3.5)); // top 3  (= 2 + 1 shift)
    }

    #[test]
    fn blob_sizes_stay_firmware_friendly() {
        for (name, toml) in [
            ("60% ANSI/ISO/split-bs", ANSI_ISO_60),
            ("Corne split (42/36)", CORNE_SPLIT),
        ] {
            let cfg: LayoutTomlConfig = toml::from_str(toml).unwrap();
            let compressed = build_layout_blob(&cfg, None).unwrap().len();
            assert!(compressed < 2048, "{name} blob {compressed} B exceeds 2 KB");
        }
    }

    #[test]
    fn corne_worked_example() {
        // The Corne walk from the design doc: first pinky key, gap jump, tilted thumb.
        let toml = r#"
rows = 4
cols = 12
layers = 1
map = """
(0,0,L,@cP) (0,1,L,@cR) (0,2,L,@cM) (0,3,L,@cI) (0,4,L,@cI) (0,5,L,@cX) [1.0] (0,6,R,@cX) (0,7,R,@cI) (0,8,R,@cI) (0,9,R,@cM) (0,10,R,@cR) (0,11,R,@cP)
(1,0,L,@cP) (1,1,L,@cR) (1,2,L,@cM) (1,3,L,@cI) (1,4,L,@cI) (1,5,L,@cX) [1.0] (1,6,R,@cX) (1,7,R,@cI) (1,8,R,@cI) (1,9,R,@cM) (1,10,R,@cR) (1,11,R,@cP)
(2,0,L,@cP) (2,1,L,@cR) (2,2,L,@cM) (2,3,L,@cI) (2,4,L,@cI) (2,5,L,@cX) [1.0] (2,6,R,@cX) (2,7,R,@cI) (2,8,R,@cI) (2,9,R,@cM) (2,10,R,@cR) (2,11,R,@cP)
[y=0.05]
[3.5] (3,3,L,@thumbL) (3,4,L) (3,5,L,@thumbR) [1.0] (3,6,R,@thumbL) (3,7,R) (3,8,R,@thumbR)
"""

[shapes]
cP = { y = 0.55 }
cR = { y = 0.25 }
cM = { y = 0.0 }
cI = { y = 0.10 }
cX = { y = 0.25 }
thumbL = { r = 15.0 }
thumbR = { r = -15.0 }
"#;
        let info = info_of(toml);
        let v = &info.variants[0];
        // (0,0,L,@cP) lands at center (0.5, 1.05).
        let k00 = key(v, 0, 0);
        assert!(approx(k00.x, 0.5) && approx(k00.y, 1.05), "got ({}, {})", k00.x, k00.y);
        // After six left keys + [1.0] gap, (0,6,R) lands at x = 7.5.
        assert!(approx(key(v, 0, 6).x, 7.5), "right half x");
        // Thumb (3,3) lands at (4.0, 3.55), tilted +15.
        let t = key(v, 3, 3);
        assert!(approx(t.x, 4.0) && approx(t.y, 3.55), "thumb ({}, {})", t.x, t.y);
        assert!(approx(t.r, 15.0));
        // 36 grid keys + 6 thumbs = 42.
        assert_eq!(v.keys.len(), 42);
    }

    #[test]
    fn iso_variant_reflows_to_match_ansi() {
        // ANSI hides the iso key (3,14) → LShift 2.25 then Z at 2.25.
        // ISO shrinks LShift to 1.25 and shows the 1u iso key → Z still at 2.25.
        let toml = r#"
rows = 4
cols = 16
layers = 1
map = """
(3,0,@2.25u) (3,14,@isokey) (3,1) (3,2)
"""

[shapes]
isokey = { w = 1.0 }
lsft_iso = { w = 1.25 }

[[variant]]
name = "ansi"
hidden = ["(3,14)"]

[[variant]]
name = "iso"
shapes = { "(3,0)" = "@lsft_iso" }
"#;
        let info = info_of(toml);
        let ansi = &info.variants[0];
        let iso = &info.variants[1];
        // (3,1) — the first alpha — lands at the same x in both variants.
        assert!(
            approx(key(ansi, 3, 1).x, key(iso, 3, 1).x),
            "ansi {} vs iso {}",
            key(ansi, 3, 1).x,
            key(iso, 3, 1).x
        );
        // ANSI omits the iso key; ISO includes it.
        assert!(ansi.keys.iter().all(|k| !(k.row == 3 && k.col == 14)));
        assert!(iso.keys.iter().any(|k| k.row == 3 && k.col == 14));
    }

    #[test]
    fn blob_round_trips_through_compression() {
        let info = info_of("rows = 1\ncols = 2\nlayers = 1\nmap = \"(0,0,@iso_enter) (0,1)\"");
        let bytes = postcard::to_allocvec(&info).unwrap();
        let compressed = miniz_oxide::deflate::compress_to_vec(&bytes, 6);
        let back = miniz_oxide::inflate::decompress_to_vec(&compressed).unwrap();
        let decoded: LayoutInfo = postcard::from_bytes(&back).unwrap();
        assert_eq!(decoded, info);
        // The ISO enter carries a second rectangle.
        assert!(decoded.variants[0].keys[0].rect2.is_some());
    }

    /// Faithful ANSI/ISO/split-bs 60% from the design doc §7: one keymap, three
    /// render variants over the superset map.
    const ANSI_ISO_60: &str = r#"
rows = 5
cols = 16
layers = 1
default_variant = "ansi"
map = """
(0,0) (0,1) (0,2) (0,3) (0,4) (0,5) (0,6) (0,7) (0,8) (0,9) (0,10) (0,11) (0,12) (0,13,@bs) (0,14,@bsr)
(1,0,@tab) (1,1) (1,2) (1,3) (1,4) (1,5) (1,6) (1,7) (1,8) (1,9) (1,10) (1,11) (1,12) (1,13)
(2,0,@caps) (2,1) (2,2) (2,3) (2,4) (2,5) (2,6) (2,7) (2,8) (2,9) (2,10) (2,11) (2,12,@enter)
(3,0,@lsft) (3,14,@isokey) (3,1) (3,2) (3,3) (3,4) (3,5) (3,6) (3,7) (3,8) (3,9) (3,10) (3,11,@rsft)
(4,0,@mod) (4,1,@mod) (4,2,@mod) (4,3,@space) (4,9,@mod) (4,10,@mod) (4,11,@mod) (4,12,@mod)
"""

[shapes]
bs = { w = 2.0 }
bsr = { w = 1.0 }
bsl = { w = 1.0 }
tab = { w = 1.5 }
caps = { w = 1.75 }
enter = { w = 2.25 }
isoenter = { w = 1.25, h = 2.0, y = -1.0, w2 = 1.5, h2 = 1.0, x2 = -0.25 }
lsft = { w = 2.25 }
lsft_iso = { w = 1.25 }
isokey = { w = 1.0 }
rsft = { w = 2.75 }
mod = { w = 1.25 }
space = { w = 6.25 }

[[variant]]
name = "ansi"
hidden = ["(3,14)", "(0,14)"]

[[variant]]
name = "iso"
shapes = { "(2,12)" = "@isoenter", "(3,0)" = "@lsft_iso" }
hidden = ["(0,14)"]

[[variant]]
name = "split-bs"
shapes = { "(0,13)" = "@bsl" }
hidden = ["(3,14)"]
"#;

    #[test]
    fn multi_variant_60_percent() {
        let info = info_of(ANSI_ISO_60);
        // Three render variants over one 63-key superset; ansi shown first.
        assert_eq!(info.variants.len(), 3);
        let names: Vec<_> = info.variants.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["ansi", "iso", "split-bs"]);
        assert_eq!(info.default_variant, 0);

        let ansi = &info.variants[0];
        let iso = &info.variants[1];
        let splitbs = &info.variants[2];

        // ANSI hides the ISO-only key and the split-bs half; ISO shows the ISO
        // key; split-bs shows the split half. None of them changes N's identity.
        let has = |v: &Variant, r, c| v.keys.iter().any(|k| k.row == r && k.col == c);
        assert!(!has(ansi, 3, 14) && !has(ansi, 0, 14));
        assert!(has(iso, 3, 14) && !has(iso, 0, 14));
        assert!(has(splitbs, 0, 14) && !has(splitbs, 3, 14));

        // ISO Enter at (2,12) is a true L (two rects) only in the iso variant.
        assert!(key(iso, 2, 12).rect2.is_some());
        assert!(key(ansi, 2, 12).rect2.is_none());
        assert!(approx(key(iso, 3, 0).w, 1.25)); // LShift shrank for the extra key

        // Reflow check: hiding (3,14) in ansi (2.25u LShift) lands the first
        // alpha (3,1) at exactly the same x as iso (1.25u LShift + shown 1u key).
        assert!(
            approx(key(ansi, 3, 1).x, key(iso, 3, 1).x),
            "row-3 alpha must align: ansi {} vs iso {}",
            key(ansi, 3, 1).x,
            key(iso, 3, 1).x
        );

        // The classic row stagger: 1.5u Tab and 1.75u Caps push their rows right,
        // so the alpha home row sits right of the number row.
        assert!(key(ansi, 1, 1).x > key(ansi, 0, 1).x);
        assert!(key(ansi, 2, 1).x > key(ansi, 1, 1).x);
    }

    #[test]
    fn multi_variant_60_blob_is_small() {
        let cfg: LayoutTomlConfig = toml::from_str(ANSI_ISO_60).unwrap();
        let blob = build_layout_blob(&cfg, None).unwrap();
        // A 3-variant 60% compresses to well under a BLE-friendly couple of KB.
        assert!(!blob.is_empty() && blob.len() < 2048, "blob len = {}", blob.len());
        // And it inflates + decodes back to the same LayoutInfo.
        let back = miniz_oxide::inflate::decompress_to_vec(&blob).unwrap();
        let decoded: LayoutInfo = postcard::from_bytes(&back).unwrap();
        assert_eq!(decoded, build_layout_info(&cfg, None).unwrap().unwrap());
    }

    #[test]
    fn example_nrf52840_numpad_geometry() {
        // The geometry from examples/use_config/nrf52840_ble/keyboard.toml: a
        // numpad whose Plus/Enter span two rows (@2uv) and whose zero is 2u wide
        // (@2u), all via stock shapes. Keeps the shipped example honest.
        let toml = r#"
rows = 5
cols = 4
layers = 2
map = """
(0,0) (0,1) (0,2) (0,3)
(1,0) (1,1) (1,2) (1,3,@2uv)
(2,0) (2,1) (2,2)
(3,0) (3,1) (3,2) (3,3,@2uv)
    (4,0,@2u)    (4,1)
"""
"#;
        let info = info_of(toml);
        let v = &info.variants[0];
        assert_eq!(v.keys.len(), 17); // 4 + 4 + 3 + 4 + 2
        // Plus and Enter are 2u tall, centered one unit below their row top.
        assert!(approx(key(v, 1, 3).h, 2.0) && approx(key(v, 1, 3).y, 2.0));
        assert!(approx(key(v, 3, 3).h, 2.0) && approx(key(v, 3, 3).y, 4.0));
        // The zero is 2u wide; the dot sits immediately right of it.
        assert!(approx(key(v, 4, 0).w, 2.0) && approx(key(v, 4, 0).x, 1.0));
        assert!(approx(key(v, 4, 1).x, 2.5));
    }

    #[test]
    fn split_corne_36_key_variant() {
        // The split Corne from §7 plus the 36-key view (hide the outer pinky
        // columns). Same matrix, no morph — the variant only drops keys.
        let info = info_of(CORNE_SPLIT);
        assert_eq!(info.variants.len(), 2);
        let full = &info.variants[0];
        let mini = &info.variants[1];
        assert_eq!(full.keys.len(), 42); // 36 grid + 6 thumbs
        assert_eq!(mini.keys.len(), 36); // outer pinky columns hidden

        // The split gap is real: the right inner column (col 6) sits a full gap
        // to the right of the left inner column (col 5) on the same row.
        assert!(key(full, 0, 6).x - key(full, 0, 5).x > 1.5);

        // Reflow: hiding the left pinky (0,0) shifts the rest of row 0 left by 1u
        // in the 36 view (the ring column is now the leftmost).
        assert!(approx(key(full, 0, 1).x - key(mini, 0, 1).x, 1.0));
    }
}
