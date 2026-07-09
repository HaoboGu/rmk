//! Build-time physical key layout.
//!
//! Walk a cursor over the `[layout].map` tokens (keys, encoders, gaps, `[y=]`
//! steps, `[r=deg@(x,y)]` rotation regions), apply each `[[layout.variant]]`
//! overlay, and produce a compressed,
//! opaque blob the firmware streams verbatim over `GetLayout` (it never decodes
//! it). The host inflates + postcard-decodes the blob into `LayoutInfo`.
//!
//! The `#[derive(Serialize)]` mirror types here MUST match the host-decode types
//! in the `rynk` host crate (`rynk::layout::*`) field-for-field. The match is by
//! hand (no shared crate: these need `alloc`, but the only common dependency
//! `rmk-types` is deliberately alloc-free), guarded by the cross-crate round-trip
//! tests in `rynk/rynk-kle/src/to_layout.rs` (which decode this builder's blob
//! back through `rynk::LayoutInfo::from_compressed_blob`).

use std::collections::{HashMap, HashSet};

use pest::Parser;
use serde::{Deserialize, Serialize};

use crate::LayoutTomlConfig;
use crate::keymap::{ConfigParser, Rule};

// ── Wire mirror types (must match the host decoder in `rynk::layout::*`) ─────

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
    rect: Rect,
    r: f32,
    rect2: Option<Rect>,
}

// A fixed 1u knob: it renders at a center only — never resized, rotated, or
// L-shaped, so it carries neither a size, an `r`, nor a `rect2`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Encoder {
    id: u8,
    x: f32,
    y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Variant {
    name: String,
    keys: Vec<Key>,
    encoders: Vec<Encoder>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct LayoutInfo {
    default_variant: u8,
    variants: Vec<Variant>,
}

// ── Shapes ───────────────────────────────────────────────────────────────────

/// A resolved shape: every default applied. `rect2` is the L-key's second
/// rectangle stored as center-relative offsets — its `x`/`y` are offsets from
/// the primary center, not absolute positions (the walk resolves them).
#[derive(Clone, Copy, Debug)]
struct Shape {
    w: f32,
    h: f32,
    x: f32,
    y: f32,
    r: f32,
    rect2: Option<Rect>,
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
        let rect2 = t.w2.map(|w2| Rect {
            w: w2,
            h: t.h2.unwrap_or(1.0),
            x: t.x2.unwrap_or(0.0),
            y: t.y2.unwrap_or(0.0),
        });
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

/// RMK's shipped stock widths (`@Nu`): N units wide, 1u tall. The single source
/// of truth shared with the KLE converter in `rynk-kle`, which matches against
/// these to emit a `@Nu` reference instead of a generated shape — the two must
/// agree on names or a token the converter emits would fail to resolve here.
pub const STOCK_WIDTHS: &[(&str, f32)] = &[
    ("1.25u", 1.25),
    ("1.5u", 1.5),
    ("1.75u", 1.75),
    ("2u", 2.0),
    ("2.25u", 2.25),
    ("2.75u", 2.75),
    ("3u", 3.0),
    ("6.25u", 6.25),
    ("7u", 7.0),
];

/// The shipped stock shapes (`@2u`, `@1.5u`, …, `@iso_enter`, …). Keyed without
/// the leading `@`. User `[layout.shapes]` entries of the same name override.
fn stock_shapes() -> HashMap<String, Shape> {
    let d = Shape::default();
    let mut m = HashMap::new();
    // Width family `@Nu` — N units wide, 1u tall.
    for &(name, w) in STOCK_WIDTHS {
        m.insert(name.to_string(), Shape { w, ..d });
    }
    // Tall: 1u wide, 2u tall (numpad + / Enter).
    m.insert("2u_tall".to_string(), Shape { h: 2.0, ..d });
    // Stepped Caps: a single 1.75u rect (the step is a 3-D detail).
    m.insert("stepped_caps".to_string(), Shape { w: 1.75, ..d });
    // ISO Enter: a 1.25×2 bar + a 1.5×1 top overhang (true L, two rects). rect2
    // offsets are center-to-center (NOT KLE's top-left x2/y2): the overhang sits on
    // the bar's upper row with the right edges flush.
    m.insert(
        "iso_enter".to_string(),
        Shape {
            w: 1.25,
            h: 2.0,
            y: -1.0,
            rect2: Some(Rect {
                w: 1.5,
                h: 1.0,
                x: -0.125,
                y: -0.5,
            }),
            ..d
        },
    );
    // Big-ass Enter: 2.25×1 bottom + 1.5×1 top one row up, right-aligned.
    m.insert(
        "bae".to_string(),
        Shape {
            w: 2.25,
            rect2: Some(Rect {
                w: 1.5,
                h: 1.0,
                x: 0.375,
                y: -1.0,
            }),
            ..d
        },
    );
    m
}

// ── Map token stream ─────────────────────────────────────────────────────────

/// One token of the `[layout].map` grammar. The single shared representation:
/// keymap resolution takes the `Key` write-order + `hand`, the render walk
/// takes every token (and ignores `hand`).
pub(crate) enum MapToken {
    Key {
        row: u8,
        col: u8,
        hand: char,
        shape: Option<String>,
    },
    Encoder {
        id: u8,
    },
    /// A `[n]` horizontal gap, in key-units.
    Gap(f32),
    /// A `[y=n]` extra vertical step for the next row.
    VStep(f32),
    /// A `[r=deg@(x,y)]` rotation region: keys and encoders after it rotate
    /// `deg` clockwise about the pivot until the next `[r=...]`. `[r=0]`
    /// (pivot optional) returns to the flat frame.
    Rot {
        deg: f32,
        px: f32,
        py: f32,
    },
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

/// Parse the `[layout].map` string into its token stream, validating that every
/// key coordinate is in bounds and unique. This is the single source of truth for
/// both keymap resolution (`get_keymap_config`) and the render walk, so the two
/// can never disagree on which positions are keys or in what order.
pub(crate) fn parse_map(map: &str, rows: u8, cols: u8) -> Result<Vec<MapToken>, String> {
    let pairs =
        ConfigParser::parse(Rule::layout_map, map).map_err(|e| format!("keyboard.toml: Error in `layout.map`: {e}"))?;
    let mut tokens = Vec::new();
    let mut seen: HashSet<(u8, u8)> = HashSet::new();
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
                    let mut hand = 'C';
                    let mut shape = None;
                    for part in it {
                        match part.as_rule() {
                            Rule::left_hand => hand = 'L',
                            Rule::right_hand => hand = 'R',
                            Rule::bilateral_hand => hand = '*',
                            Rule::shape_ref => shape = Some(shape_name_of(part)),
                            _ => {}
                        }
                    }
                    if row >= rows || col >= cols {
                        return Err(format!(
                            "keyboard.toml: layout.map coordinate ({row},{col}) is out of bounds ([0..{}], [0..{}])",
                            rows.saturating_sub(1),
                            cols.saturating_sub(1)
                        ));
                    }
                    if !seen.insert((row, col)) {
                        return Err(format!(
                            "keyboard.toml: duplicate coordinate ({row},{col}) in layout.map"
                        ));
                    }
                    tokens.push(MapToken::Key { row, col, hand, shape });
                }
                Rule::encoder_info => {
                    let id = parse_u8(
                        inner.into_inner().next().ok_or("missing encoder id")?.as_str(),
                        "encoder id",
                    )?;
                    tokens.push(MapToken::Encoder { id });
                }
                Rule::spacer => {
                    let u = inner.into_inner().next().ok_or("missing gap")?.as_str();
                    tokens.push(MapToken::Gap(parse_f32(u, "gap")?));
                }
                Rule::vertical => {
                    let u = inner.into_inner().next().ok_or("missing y-step")?.as_str();
                    tokens.push(MapToken::VStep(parse_f32(u, "y-step")?));
                }
                Rule::rotation => {
                    let vals = inner
                        .into_inner()
                        .map(|p| parse_f32(p.as_str(), "rotation"))
                        .collect::<Result<Vec<f32>, String>>()?;
                    let (deg, px, py) = match vals.as_slice() {
                        [deg, px, py] => (*deg, *px, *py),
                        [deg] if *deg == 0.0 => (0.0, 0.0, 0.0),
                        [deg] => {
                            return Err(format!(
                                "keyboard.toml: [r={deg}] in layout.map needs a pivot — write \
                                 [r={deg}@(x,y)]; only [r=0] (end of region) may omit it"
                            ));
                        }
                        _ => return Err("keyboard.toml: malformed [r=...] in layout.map".to_string()),
                    };
                    if ![deg, px, py].iter().all(|v| v.is_finite()) {
                        return Err("keyboard.toml: non-finite value in layout.map [r=...]".to_string());
                    }
                    tokens.push(MapToken::Rot { deg, px, py });
                }
                Rule::newline => tokens.push(MapToken::Newline),
                _ => {}
            }
        }
    }
    Ok(tokens)
}

// ── The cursor walk ──────────────────────────────────────────────────────────

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
/// (so following keys reflow). Returns this variant's keys and encoders — a
/// hidden key before an encoder reflows the encoder along with the keys.
fn walk(
    tokens: &[MapToken],
    shapes: &HashMap<String, Shape>,
    overrides: &HashMap<(u8, u8), String>,
    hidden: &HashSet<(u8, u8)>,
) -> Result<(Vec<Key>, Vec<Encoder>), String> {
    let mut w = Walker::new();
    let mut keys = Vec::new();
    let mut encoders = Vec::new();
    // The active `[r=deg@(px,py)]` region. The cursor walk itself stays flat;
    // only emitted centers are swung about the pivot (clockwise, y-down frame).
    let mut rot: Option<(f32, f32, f32)> = None;
    let swing = |x: f32, y: f32, rot: &Option<(f32, f32, f32)>| -> (f32, f32) {
        match rot {
            None => (x, y),
            Some((deg, px, py)) => {
                let (sin, cos) = deg.to_radians().sin_cos();
                let (dx, dy) = (x - px, y - py);
                (px + dx * cos - dy * sin, py + dx * sin + dy * cos)
            }
        }
    };
    for tok in tokens {
        match tok {
            MapToken::Newline => {
                if w.row_has_content {
                    w.break_pending = true;
                }
            }
            MapToken::VStep(n) => {
                // `[y=n]` adjusts the gap above the NEXT row break. A marker with no
                // row to attach to (e.g. before the first key) has nothing to adjust,
                // so drop it rather than leak it into a later break.
                if w.row_has_content {
                    w.pending_vstep += n;
                }
            }
            MapToken::Gap(g) => {
                w.advance_if_pending();
                w.cursor_x += g;
            }
            MapToken::Key { row, col, shape, .. } => {
                w.advance_if_pending();
                // Hidden: removed from the walk — advances nothing, so following keys reflow.
                if hidden.contains(&(*row, *col)) {
                    continue;
                }
                let name = overrides.get(&(*row, *col)).map(String::as_str).or(shape.as_deref());
                let s = resolve_shape(name, shapes)?;
                let (cx, cy) = swing(w.cursor_x + s.w / 2.0 + s.x, w.baseline_y + s.h / 2.0 + s.y, &rot);
                // rect2 offsets stay in the key's own frame: the renderer tilts the
                // whole key (rect2 included) by `r` about the primary center, and
                // 2-D rotations compose additively, so region + shape angles just add.
                let rect2 = s.rect2.map(|r2| Rect {
                    x: cx + r2.x,
                    y: cy + r2.y,
                    w: r2.w,
                    h: r2.h,
                });
                keys.push(Key {
                    row: *row,
                    col: *col,
                    rect: Rect {
                        x: cx,
                        y: cy,
                        w: s.w,
                        h: s.h,
                    },
                    r: s.r + rot.map_or(0.0, |(deg, ..)| deg),
                    rect2,
                });
                w.cursor_x += s.w;
                w.row_has_content = true;
            }
            MapToken::Encoder { id } => {
                w.advance_if_pending();
                // A fixed 1u knob: centered in a 1u cell, advancing one unit.
                let (x, y) = swing(w.cursor_x + 0.5, w.baseline_y + 0.5, &rot);
                encoders.push(Encoder { id: *id, x, y });
                w.cursor_x += 1.0;
                w.row_has_content = true;
            }
            MapToken::Rot { deg, px, py } => {
                // Pure overlay state — no cursor interaction, so a marker on its
                // own line consumes no row (mirroring `[y=n]`).
                rot = (*deg != 0.0).then_some((*deg, *px, *py));
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
            .map_or(true, |r| [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()))
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
    let tokens = parse_map(map, layout.rows, layout.cols)?;

    // Real key coordinates (already bounds-checked + de-duped by `parse_map`);
    // variant overlays are validated against this set below.
    let key_coords: HashSet<(u8, u8)> = tokens
        .iter()
        .filter_map(|tok| match tok {
            MapToken::Key { row, col, .. } => Some((*row, *col)),
            _ => None,
        })
        .collect();

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
    // A variant `shapes`/`hidden` target that names no real key would be a
    // silent no-op — reject it, mirroring the unknown-shape error.
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

    // Each variant is a complete layout — its own keys AND encoders. A hidden key
    // before an encoder reflows the keys after it, the encoder included. With no
    // variants, one "default" variant carries the whole layout.
    let mut walked: Vec<(String, Vec<Key>, Vec<Encoder>)> = Vec::new();
    if variants_toml.is_empty() {
        let (keys, encoders) = walk(&tokens, &shapes, &HashMap::new(), &HashSet::new())?;
        walked.push(("default".to_string(), keys, encoders));
    } else {
        for v in variants_toml {
            let overrides = parse_overrides(&v.shapes)?;
            let hidden = parse_hidden(&v.hidden)?;
            let (keys, encoders) = walk(&tokens, &shapes, &overrides, &hidden)?;
            walked.push((v.name.clone(), keys, encoders));
        }
    }

    // Resolve by name to a 0-based index; an absent or unknown name falls back to variant 0.
    let default_variant = layout
        .default_variant
        .as_ref()
        .and_then(|name| walked.iter().position(|(n, ..)| n == name))
        .unwrap_or(0) as u8;

    // Encoder ids/count are variant-invariant (overlays never touch encoders, only
    // their positions reflow), so validate one variant's list: ids unique + dense
    // (0..N), and if any encoder positions are given they must cover every board encoder.
    let encoders = &walked[0].2;
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

    let variants = walked
        .into_iter()
        .map(|(name, keys, encoders)| Variant { name, keys, encoders })
        .collect();
    Ok(Some(LayoutInfo {
        default_variant,
        variants,
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
        postcard::to_allocvec(&info).map_err(|e| format!("keyboard.toml: layout blob serialize failed: {e}"))?;
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
        let info = info_of("rows = 1\ncols = 3\nmap = \"(0,0) (0,1) (0,2)\"");
        let v = &info.variants[0];
        assert_eq!(v.name, "default");
        for (i, k) in v.keys.iter().enumerate() {
            assert!(approx(k.rect.x, i as f32 + 0.5), "center x");
            assert!(approx(k.rect.y, 0.5), "center y");
            assert!(approx(k.rect.w, 1.0) && approx(k.rect.h, 1.0));
            assert!(k.rect2.is_none());
        }
    }

    #[test]
    fn duplicate_coord_is_rejected() {
        // The blob path now rejects a repeated (row,col) too (matching
        // get_keymap_config), instead of emitting two keys for one cell.
        let cfg: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 2\nmap = \"(0,0) (0,0)\"").unwrap();
        assert!(build_layout_blob(&cfg, None).is_err(), "duplicate (0,0) must fail");
    }

    #[test]
    fn stock_width_moves_the_cursor() {
        // A 2u key then a 1u key: 2u is centered at 1.0, next at 2.5.
        let info = info_of("rows = 1\ncols = 2\nmap = \"(0,0,@2u) (0,1)\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 0, 0).rect.x, 1.0) && approx(key(v, 0, 0).rect.w, 2.0));
        assert!(approx(key(v, 0, 1).rect.x, 2.5));
    }

    #[test]
    fn stock_iso_enter_is_a_true_l() {
        // The 1.5u overhang sits on the upper of the bar's two rows with the
        // right edges flush — rect2 offsets are center-to-center, so KLE's
        // top-left (-0.25, 0) becomes (-0.125, -0.5) here.
        let info = info_of("rows = 1\ncols = 1\nmap = \"(0,0,@iso_enter)\"");
        let k = key(&info.variants[0], 0, 0);
        let r2 = k.rect2.expect("two rects");
        assert!(approx(r2.w, 1.5) && approx(r2.h, 1.0));
        assert!(
            approx(k.rect.x + k.rect.w / 2.0, r2.x + r2.w / 2.0),
            "right edges flush: bar {} vs overhang {}",
            k.rect.x + k.rect.w / 2.0,
            r2.x + r2.w / 2.0
        );
        assert!(
            approx(r2.y, k.rect.y - 0.5),
            "overhang on the upper row: {} vs {}",
            r2.y,
            k.rect.y - 0.5
        );
    }

    #[test]
    fn y_step_is_one_shot_and_lazy() {
        // Row 1 lands 1 + 0.25 = 1.25 below row 0 despite the marker on its own line.
        let info = info_of("rows = 2\ncols = 2\nmap = \"\"\"\n(0,0) (0,1)\n[y=0.25]\n(1,0) (1,1)\n\"\"\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 0, 0).rect.y, 0.5));
        assert!(approx(key(v, 1, 0).rect.y, 1.75)); // 0.5 + 1.25
    }

    #[test]
    fn leading_y_step_is_dropped() {
        // A `[y=n]` before the first row has no preceding row to push off — it
        // must NOT leak into the row 0 → row 1 break and shift the whole board.
        let info = info_of("rows = 3\ncols = 1\nmap = \"\"\"\n[y=0.5]\n(0,0)\n(1,0)\n(2,0)\n\"\"\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 0, 0).rect.y, 0.5));
        assert!(approx(key(v, 1, 0).rect.y, 1.5)); // not 2.0
        assert!(approx(key(v, 2, 0).rect.y, 2.5)); // not 3.0
    }

    #[test]
    fn unknown_default_variant_falls_back_to_zero() {
        // Per decision #6: an unknown (or absent) default_variant resolves to 0,
        // it does not fail the build.
        let info = info_of(
            "rows = 1\ncols = 1\ndefault_variant = \"typo\"\nmap = \"(0,0)\"\n[[variant]]\nname = \"a\"\n[[variant]]\nname = \"b\"",
        );
        assert_eq!(info.default_variant, 0);
        // A board with no [[variant]] at all + a named default still resolves to 0.
        let info2 = info_of("rows = 1\ncols = 1\ndefault_variant = \"x\"\nmap = \"(0,0)\"");
        assert_eq!(info2.default_variant, 0);
    }

    #[test]
    fn encoder_ids_must_be_unique_and_dense() {
        let ok = info_of("rows = 1\ncols = 2\nmap = \"(0,0) (e,0) (0,1) (e,1)\"");
        assert_eq!(ok.variants[0].encoders.len(), 2);
        let dup: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0) (e,0) (e,0)\"").unwrap();
        assert!(build_layout_info(&dup, None).is_err(), "duplicate encoder id must fail");
        let gap: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0) (e,0) (e,2)\"").unwrap();
        assert!(
            build_layout_info(&gap, None).is_err(),
            "non-dense encoder ids must fail"
        );
    }

    #[test]
    fn encoder_shape_is_rejected() {
        // An encoder is a fixed 1u knob: `(e,id,@shape)` is not valid syntax.
        let cfg: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0) (e,0,@2u)\"").unwrap();
        assert!(build_layout_blob(&cfg, None).is_err(), "(e,id,@shape) must be rejected");
    }

    #[test]
    fn out_of_bounds_coord_is_rejected() {
        let cfg: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0) (0,5)\"").unwrap();
        assert!(build_layout_info(&cfg, None).is_err());
    }

    #[test]
    fn non_finite_shape_is_rejected() {
        let nan: LayoutTomlConfig =
            toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0,@bad)\"\n[shapes]\nbad = { w = nan }").unwrap();
        assert!(build_layout_info(&nan, None).is_err(), "nan width must fail");
        let inf: LayoutTomlConfig =
            toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0,@big)\"\n[shapes]\nbig = { x = inf }").unwrap();
        assert!(build_layout_info(&inf, None).is_err(), "inf nudge must fail");
    }

    #[test]
    fn variant_target_must_be_a_real_key() {
        // `hidden` names (0,9), which is not a key in the 1x2 map.
        let cfg: LayoutTomlConfig = toml::from_str(
            "rows = 1\ncols = 2\nmap = \"(0,0) (0,1)\"\n[[variant]]\nname = \"a\"\nhidden = [\"(0,9)\"]",
        )
        .unwrap();
        assert!(build_layout_info(&cfg, None).is_err());
    }

    #[test]
    fn encoders_reflow_per_variant() {
        // `mini` hides (0,0) and reflows the tokens after it left — the encoder
        // included: the knob sits at x=2.5 in `full` but x=1.5 in `mini`.
        let info = info_of(
            "rows = 1\ncols = 2\nmap = \"(0,0) (0,1) (e,0)\"\n[[variant]]\nname = \"full\"\n[[variant]]\nname = \"mini\"\nhidden = [\"(0,0)\"]",
        );
        let full = info.variants.iter().find(|v| v.name == "full").unwrap();
        let mini = info.variants.iter().find(|v| v.name == "mini").unwrap();
        assert_eq!(full.encoders.len(), 1);
        assert!(approx(full.encoders[0].x, 2.5), "full knob x = {}", full.encoders[0].x);
        assert!(
            approx(mini.encoders[0].x, 1.5),
            "mini knob x = {} (reflowed after hiding (0,0))",
            mini.encoders[0].x
        );
        assert!(
            mini.keys.iter().all(|k| !(k.row == 0 && k.col == 0)),
            "(0,0) is hidden in mini"
        );
    }

    #[test]
    fn encoder_count_must_match_board() {
        // 1 `(e,id)` token but the board declares 2 encoders → error.
        let one: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0) (e,0)\"").unwrap();
        assert!(
            build_layout_blob(&one, Some(2)).is_err(),
            "1 token vs 2 board encoders must fail"
        );
        assert!(build_layout_blob(&one, Some(1)).is_ok(), "matching count is fine");
        // Providing NO encoder positions on a board that has encoders is allowed.
        let none: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nmap = \"(0,0)\"").unwrap();
        assert!(
            build_layout_blob(&none, Some(3)).is_ok(),
            "opting out of encoder positions is allowed"
        );
    }

    const CORNE_SPLIT: &str = r#"
rows = 4
cols = 12
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
    fn rotation_region_swings_keys_about_the_pivot() {
        // [r=90@(2.5,0)]: (0,1) flat center (3.0, 0.5) swings to (2.0, 0.5);
        // (0,2) flat (4.0, 0.5) lands one unit further down the same vertical.
        let info = info_of("rows = 1\ncols = 3\nmap = \"(0,0) [1.5] [r=90@(2.5,0)] (0,1) (0,2)\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 0, 0).rect.x, 0.5) && approx(key(v, 0, 0).r, 0.0));
        let k1 = key(v, 0, 1);
        assert!(
            approx(k1.rect.x, 2.0) && approx(k1.rect.y, 0.5) && approx(k1.r, 90.0),
            "k1 ({}, {}, r={})",
            k1.rect.x,
            k1.rect.y,
            k1.r
        );
        let k2 = key(v, 0, 2);
        assert!(
            approx(k2.rect.x, 2.0) && approx(k2.rect.y, 1.5) && approx(k2.r, 90.0),
            "k2 ({}, {}, r={})",
            k2.rect.x,
            k2.rect.y,
            k2.r
        );
    }

    #[test]
    fn rotation_is_rigid_across_rows() {
        // A 2x2 block in one region: both lines share the pivot, so the flat
        // inter-key geometry survives the rotation unchanged (rigid transform).
        let info =
            info_of("rows = 2\ncols = 2\nmap = \"\"\"\n[3.5] [r=25@(3.5,0)] (0,0) (0,1)\n[3.5] (1,0) (1,1)\n\"\"\"");
        let v = &info.variants[0];
        assert!(v.keys.iter().all(|k| approx(k.r, 25.0)), "all keys carry the angle");
        let d = |a: &Key, b: &Key| ((a.rect.x - b.rect.x).powi(2) + (a.rect.y - b.rect.y).powi(2)).sqrt();
        assert!(approx(d(key(v, 0, 0), key(v, 0, 1)), 1.0));
        assert!(approx(d(key(v, 0, 0), key(v, 1, 0)), 1.0));
        assert!(approx(d(key(v, 0, 0), key(v, 1, 1)), 2f32.sqrt()));
        // And the block really rotated: (0,0) center = pivot + R(25°)·(0.5, 0.5).
        let (sin, cos) = 25f32.to_radians().sin_cos();
        let k = key(v, 0, 0);
        assert!(
            approx(k.rect.x, 3.5 + 0.5 * cos - 0.5 * sin) && approx(k.rect.y, 0.5 * sin + 0.5 * cos),
            "got ({}, {})",
            k.rect.x,
            k.rect.y
        );
    }

    #[test]
    fn rotation_zero_ends_the_region() {
        // Mid-line: after [r=0] the key returns to its flat cursor spot.
        let info = info_of("rows = 1\ncols = 3\nmap = \"(0,0) [r=15@(1,0)] (0,1) [r=0] (0,2)\"");
        let v = &info.variants[0];
        let k2 = key(v, 0, 2);
        assert!(approx(k2.rect.x, 2.5) && approx(k2.rect.y, 0.5) && approx(k2.r, 0.0));
        // Across lines too: the region does not leak into the next row.
        let info = info_of("rows = 2\ncols = 1\nmap = \"\"\"\n[r=30@(0,0)] (0,0)\n[r=0] (1,0)\n\"\"\"");
        let v = &info.variants[0];
        assert!(approx(key(v, 1, 0).rect.x, 0.5) && approx(key(v, 1, 0).rect.y, 1.5));
    }

    #[test]
    fn rotation_composes_with_shape_r_and_swings_encoders() {
        let toml = "rows = 1\ncols = 1\nmap = \"[r=15@(0,0)] (0,0,@tilt) (e,0)\"\n[shapes]\ntilt = { r = 10.0 }";
        let info = info_of(toml);
        let v = &info.variants[0];
        // Region and shape angles add: the key spins 10° in place within a
        // cluster that is itself rotated 15°.
        assert!(approx(key(v, 0, 0).r, 25.0), "r = {}", key(v, 0, 0).r);
        // The knob's flat center (1.5, 0.5) swings about the origin.
        let (sin, cos) = 15f32.to_radians().sin_cos();
        let e = &v.encoders[0];
        assert!(
            approx(e.x, 1.5 * cos - 0.5 * sin) && approx(e.y, 1.5 * sin + 0.5 * cos),
            "knob ({}, {})",
            e.x,
            e.y
        );
    }

    #[test]
    fn rotation_without_pivot_is_rejected() {
        let cfg: LayoutTomlConfig = toml::from_str("rows = 1\ncols = 1\nmap = \"[r=15] (0,0)\"").unwrap();
        let err = build_layout_info(&cfg, None).unwrap_err();
        assert!(err.contains("pivot"), "{err}");
    }

    #[test]
    fn hidden_keys_reflow_inside_a_rotation_region() {
        // Hiding (0,0) reflows (0,1) to the region start in the FLAT frame
        // first, then the rotation applies — so the mini variant's (0,1) lands
        // exactly where the full variant's (0,0) was.
        let toml = "rows = 1\ncols = 2\nmap = \"[1.0] [r=20@(1,0)] (0,0) (0,1)\"\n[[variant]]\nname = \"full\"\n[[variant]]\nname = \"mini\"\nhidden = [\"(0,0)\"]";
        let info = info_of(toml);
        let full = info.variants.iter().find(|v| v.name == "full").unwrap();
        let mini = info.variants.iter().find(|v| v.name == "mini").unwrap();
        let (a, b) = (&key(full, 0, 0).rect, &key(mini, 0, 1).rect);
        assert!(
            approx(a.x, b.x) && approx(a.y, b.y),
            "({}, {}) vs ({}, {})",
            a.x,
            a.y,
            b.x,
            b.y
        );
    }

    #[test]
    fn y_step_shifts_every_row_below() {
        // `[y=1]` between row 0 and row 1 pushes the gap by +1; because the
        // baseline accumulates, EVERY row below row 0 is shifted down by 1.
        let info = info_of("rows = 3\ncols = 1\nmap = \"\"\"\n(0,0)\n[y=1]\n(1,0)\n(2,0)\n\"\"\"");
        let v = &info.variants[0];
        // Stored y is the key CENTER (row top + 0.5). row-tops: 0, 2, 3.
        assert!(approx(key(v, 0, 0).rect.y, 0.5)); // top 0
        assert!(approx(key(v, 1, 0).rect.y, 2.5)); // top 2  (= 1 + 1 shift)
        assert!(approx(key(v, 2, 0).rect.y, 3.5)); // top 3  (= 2 + 1 shift)
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
        // Corne walk: first pinky key, gap jump, tilted thumb.
        let toml = r#"
rows = 4
cols = 12
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
        assert!(
            approx(k00.rect.x, 0.5) && approx(k00.rect.y, 1.05),
            "got ({}, {})",
            k00.rect.x,
            k00.rect.y
        );
        // After six left keys + [1.0] gap, (0,6,R) lands at x = 7.5.
        assert!(approx(key(v, 0, 6).rect.x, 7.5), "right half x");
        // Thumb (3,3) lands at (4.0, 3.55), tilted +15.
        let t = key(v, 3, 3);
        assert!(
            approx(t.rect.x, 4.0) && approx(t.rect.y, 3.55),
            "thumb ({}, {})",
            t.rect.x,
            t.rect.y
        );
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
            approx(key(ansi, 3, 1).rect.x, key(iso, 3, 1).rect.x),
            "ansi {} vs iso {}",
            key(ansi, 3, 1).rect.x,
            key(iso, 3, 1).rect.x
        );
        // ANSI omits the iso key; ISO includes it.
        assert!(ansi.keys.iter().all(|k| !(k.row == 3 && k.col == 14)));
        assert!(iso.keys.iter().any(|k| k.row == 3 && k.col == 14));
    }

    #[test]
    fn blob_round_trips_through_compression() {
        let info = info_of("rows = 1\ncols = 2\nmap = \"(0,0,@iso_enter) (0,1)\"");
        let bytes = postcard::to_allocvec(&info).unwrap();
        let compressed = miniz_oxide::deflate::compress_to_vec(&bytes, 6);
        let back = miniz_oxide::inflate::decompress_to_vec(&compressed).unwrap();
        let decoded: LayoutInfo = postcard::from_bytes(&back).unwrap();
        assert_eq!(decoded, info);
        // The ISO enter carries a second rectangle.
        assert!(decoded.variants[0].keys[0].rect2.is_some());
    }

    /// ANSI/ISO/split-bs 60%: one keymap, three render variants over the superset map.
    const ANSI_ISO_60: &str = r#"
rows = 5
cols = 16
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
isoenter = { w = 1.25, h = 2.0, y = -1.0, w2 = 1.5, h2 = 1.0, x2 = -0.125, y2 = -0.5 }
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
        assert!(approx(key(iso, 3, 0).rect.w, 1.25)); // LShift shrank for the extra key

        // Reflow check: hiding (3,14) in ansi (2.25u LShift) lands the first
        // alpha (3,1) at exactly the same x as iso (1.25u LShift + shown 1u key).
        assert!(
            approx(key(ansi, 3, 1).rect.x, key(iso, 3, 1).rect.x),
            "row-3 alpha must align: ansi {} vs iso {}",
            key(ansi, 3, 1).rect.x,
            key(iso, 3, 1).rect.x
        );

        // The classic row stagger: 1.5u Tab and 1.75u Caps push their rows right,
        // so the alpha home row sits right of the number row.
        assert!(key(ansi, 1, 1).rect.x > key(ansi, 0, 1).rect.x);
        assert!(key(ansi, 2, 1).rect.x > key(ansi, 1, 1).rect.x);
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
    fn example_nrf52840_numpad_layout() {
        // The layout from examples/use_config/nrf52840_ble/keyboard.toml: a
        // numpad whose Plus/Enter span two rows (@2u_tall) and whose zero is 2u wide
        // (@2u), all via stock shapes. Keeps the shipped example honest.
        let toml = r#"
rows = 5
cols = 4
map = """
(0,0) (0,1) (0,2) (0,3)
(1,0) (1,1) (1,2) (1,3,@2u_tall)
(2,0) (2,1) (2,2)
(3,0) (3,1) (3,2) (3,3,@2u_tall)
    (4,0,@2u)    (4,1)
"""
"#;
        let info = info_of(toml);
        let v = &info.variants[0];
        assert_eq!(v.keys.len(), 17); // 4 + 4 + 3 + 4 + 2
        // Plus and Enter are 2u tall, centered one unit below their row top.
        assert!(approx(key(v, 1, 3).rect.h, 2.0) && approx(key(v, 1, 3).rect.y, 2.0));
        assert!(approx(key(v, 3, 3).rect.h, 2.0) && approx(key(v, 3, 3).rect.y, 4.0));
        // The zero is 2u wide; the dot sits immediately right of it.
        assert!(approx(key(v, 4, 0).rect.w, 2.0) && approx(key(v, 4, 0).rect.x, 1.0));
        assert!(approx(key(v, 4, 1).rect.x, 2.5));
    }

    #[test]
    fn split_corne_36_key_variant() {
        // The split Corne, plus the 36-key view (hide the outer pinky
        // columns). Same matrix, no morph — the variant only drops keys.
        let info = info_of(CORNE_SPLIT);
        assert_eq!(info.variants.len(), 2);
        let full = &info.variants[0];
        let mini = &info.variants[1];
        assert_eq!(full.keys.len(), 42); // 36 grid + 6 thumbs
        assert_eq!(mini.keys.len(), 36); // outer pinky columns hidden

        // The split gap is real: the right inner column (col 6) sits a full gap
        // to the right of the left inner column (col 5) on the same row.
        assert!(key(full, 0, 6).rect.x - key(full, 0, 5).rect.x > 1.5);

        // Reflow: hiding the left pinky (0,0) shifts the rest of row 0 left by 1u
        // in the 36 view (the ring column is now the leftmost).
        assert!(approx(key(full, 0, 1).rect.x - key(mini, 0, 1).rect.x, 1.0));
    }
}
