//! Deserialize a KLE keymap — the bare row array of a KLE JSON export, or the
//! `layouts.keymap` inside a Vial/VIA keyboard definition (same format).
//!
//! The heavy lifting — the cursor walk that turns KLE's relative offsets into
//! absolute key positions, plus legend normalization — is done by the upstream
//! [`kle_serial`] crate. Here we only add the annotations the layout generator
//! needs beside each `kle_serial::Key`: the two meaningful VIA legends
//! (top-left = `row,col`, any other `n,n` = a `group,choice` layout option) and
//! the source row each key came from (which `kle_serial` flattens away, but we
//! need it to rebuild map lines).

use serde_json::Value;

/// RMK/Vial meaning attached to a `kle_serial::Key`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyAnnotation {
    /// Matrix position from the top-left legend (`None` if it isn't `row,col`, or
    /// if this is an encoder switch).
    pub matrix: Option<(u32, u32)>,
    /// `(encoder index, rotary action)` when this key is a Vial encoder switch
    /// (marked by a center legend of `e`); action `0` is CCW, `1` is CW.
    pub encoder: Option<(u32, u32)>,
    /// `group,choice` layout option from any other legend, if present.
    pub option: Option<(u32, u32)>,
    /// Index of the source keymap row — used to reconstruct RMK map lines.
    pub row_index: usize,
}

/// KLE keys from `kle-serial`, plus RMK/Vial annotations by the same index.
#[derive(Clone, Debug)]
pub struct ParsedKeymap {
    pub keys: Vec<kle_serial::Key<f64>>,
    pub annotations: Vec<KeyAnnotation>,
}

impl ParsedKeymap {
    pub fn has_matrix_or_encoder(&self) -> bool {
        self.annotations
            .iter()
            .any(|a| a.matrix.is_some() || a.encoder.is_some())
    }
}

/// A legend of the form `int,int` — the matrix position (top-left) or a
/// `group,choice` layout option (any other slot).
fn parse_rc(label: &str) -> Option<(u32, u32)> {
    let (a, b) = label.trim().split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// Parse the KLE `keymap` array into a flat list of placed keys plus annotations.
pub fn parse_keymap(keymap: &Value) -> Result<ParsedKeymap, String> {
    let keyboard: kle_serial::Keyboard<f64> =
        serde_json::from_value(keymap.clone()).map_err(|e| format!("failed to parse KLE layout: {e}"))?;

    // `kle_serial` emits keys in reading order (row-major); recover each key's
    // source row by counting the string items in each raw row in the same order.
    let mut row_of_key: Vec<usize> = Vec::new();
    if let Some(rows) = keymap.as_array() {
        for (row_index, row) in rows.iter().enumerate() {
            if let Some(items) = row.as_array() {
                for item in items {
                    if item.is_string() {
                        row_of_key.push(row_index);
                    }
                }
            }
        }
    }

    let mut annotations = Vec::with_capacity(keyboard.keys.len());
    for (i, k) in keyboard.keys.iter().enumerate() {
        // A Vial encoder switch carries a center legend `e`; then the top-left
        // legend is `index,action` (not a matrix cell).
        let top_left = k.legends[0].as_ref().and_then(|l| parse_rc(&l.text));
        let is_encoder = k.legends.iter().skip(1).flatten().any(|l| l.text.trim() == "e");
        let (matrix, encoder) = if is_encoder { (None, top_left) } else { (top_left, None) };
        let option = k.legends.iter().skip(1).flatten().find_map(|l| parse_rc(&l.text));
        annotations.push(KeyAnnotation {
            matrix,
            encoder,
            option,
            row_index: row_of_key.get(i).copied().unwrap_or(0),
        });
    }
    Ok(ParsedKeymap {
        keys: keyboard.keys,
        annotations,
    })
}

/// Fallback for a plain KLE export, where legends are key labels rather than VIA
/// `row,col` markers: assign matrix positions row-major — row = the key's source
/// KLE row (renumbered densely), col = its order within that row. Decals and
/// encoder switches are left alone.
pub fn assign_matrix_by_position(keymap: &mut ParsedKeymap) {
    let mut rows: Vec<usize> = Vec::new();
    let mut next_col: Vec<u32> = Vec::new();
    for (key, annotation) in keymap.keys.iter().zip(&mut keymap.annotations) {
        if key.decal || annotation.encoder.is_some() {
            continue;
        }
        let row = rows.iter().position(|&r| r == annotation.row_index).unwrap_or_else(|| {
            rows.push(annotation.row_index);
            next_col.push(0);
            rows.len() - 1
        });
        annotation.matrix = Some((row as u32, next_col[row]));
        next_col[row] += 1;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn keys_of(v: Value) -> ParsedKeymap {
        parse_keymap(&v).unwrap()
    }

    #[test]
    fn plain_grid_positions_and_matrix() {
        let k = keys_of(json!([["0,0", "0,1"], ["1,0", "1,1"]]));
        assert_eq!(k.keys.len(), 4);
        assert_eq!(k.annotations[0].matrix, Some((0, 0)));
        assert_eq!((k.keys[0].x, k.keys[0].y), (0.0, 0.0));
        assert_eq!((k.keys[1].x, k.keys[1].y), (1.0, 0.0));
        assert_eq!(k.annotations[2].matrix, Some((1, 0)));
        assert_eq!((k.keys[2].x, k.keys[2].y), (0.0, 1.0));
        assert_eq!(k.annotations[3].row_index, 1);
    }

    #[test]
    fn width_advances_cursor() {
        let k = keys_of(json!([[{"w": 2.0}, "0,0", "0,1"]]));
        assert_eq!(k.keys[0].width, 2.0);
        assert_eq!(k.keys[1].x, 2.0); // next key starts after the 2u key
        assert_eq!(k.keys[1].width, 1.0); // width resets to 1 after each key
    }

    #[test]
    fn relative_x_y_offsets() {
        // Vial's own rp2040 map: the last two keys jump up 2 rows and right 4.
        let k = keys_of(json!([
            ["0,0", "0,1", "0,2"],
            ["1,0", "1,1", "1,2"],
            ["2,0", "2,1", "2,2"],
            [{"y": -2.0, "x": 4.0}, "3,0", "3,2"],
        ]));
        let last = k.keys.len() - 2; // "3,0"
        assert_eq!(k.annotations[last].matrix, Some((3, 0)));
        assert_eq!((k.keys[last].x, k.keys[last].y), (4.0, 1.0)); // row3 baseline 3, -2 => 1
    }

    #[test]
    fn layout_option_label_is_parsed() {
        // Top-left is the matrix; a trailing `n,n` legend is group,choice.
        let k = keys_of(json!([["0,13\n\n\n\n\n\n\n\n\n0,1"]]));
        assert_eq!(k.annotations[0].matrix, Some((0, 13)));
        assert_eq!(k.annotations[0].option, Some((0, 1)));
    }

    #[test]
    fn decal_is_flagged() {
        let k = keys_of(json!([[{"d": true}, "0,0", "0,1"]]));
        assert!(k.keys[0].decal);
        assert!(!k.keys[1].decal); // decal resets after the key
    }

    #[test]
    fn iso_enter_is_an_l_shape() {
        let k = keys_of(json!([[{"w": 1.25, "h": 2.0, "w2": 1.5, "h2": 1.0, "x2": -0.25}, "2,12"]]));
        assert_eq!(k.keys[0].x2, -0.25);
        assert_eq!(k.keys[0].width2, 1.5);
    }

    #[test]
    fn kle_export_metadata_object_is_skipped() {
        // A KLE "Download JSON" leads with a metadata object; keys are unaffected.
        let k = keys_of(json!([{"name": "sixty"}, ["0,0", "0,1"], ["1,0"]]));
        assert_eq!(k.keys.len(), 3);
        assert_eq!(k.annotations[0].matrix, Some((0, 0)));
        assert_eq!((k.keys[0].x, k.keys[0].y), (0.0, 0.0));
        assert_eq!((k.keys[2].x, k.keys[2].y), (0.0, 1.0));
    }

    #[test]
    fn plain_legends_get_row_major_matrix() {
        // Label legends parse to no matrix; the fallback assigns row-major
        // positions with rows renumbered densely past the metadata object.
        let mut k = keys_of(json!([
            {"name": "plain"},
            ["Esc", "Q", "W"],
            [{"w": 1.5}, "Tab", {"d": true}, "logo", "A"],
        ]));
        assert!(
            k.annotations
                .iter()
                .all(|annotation| annotation.matrix.is_none() && annotation.encoder.is_none())
        );
        assign_matrix_by_position(&mut k);
        assert_eq!(k.annotations[0].matrix, Some((0, 0)));
        assert_eq!(k.annotations[2].matrix, Some((0, 2)));
        assert_eq!(k.annotations[3].matrix, Some((1, 0))); // Tab
        assert_eq!(k.annotations[4].matrix, None); // decal stays unplaced
        assert_eq!(k.annotations[5].matrix, Some((1, 1))); // A: the decal takes no column
    }

    #[test]
    fn encoder_switch_is_not_a_matrix_key() {
        // Vial's own encoder syntax: center legend `e`, top-left is `index,action`.
        let k = keys_of(json!([["0,0\n\n\n\n\n\n\n\n\ne", "0,1\n\n\n\n\n\n\n\n\ne"]]));
        assert_eq!(k.annotations[0].matrix, None);
        assert_eq!(k.annotations[0].encoder, Some((0, 0))); // encoder 0, CCW
        assert_eq!(k.annotations[1].encoder, Some((0, 1))); // encoder 0, CW
        // A plain key with the same top-left legend stays a matrix key.
        let plain = keys_of(json!([["0,0"]]));
        assert_eq!(plain.annotations[0].matrix, Some((0, 0)));
        assert_eq!(plain.annotations[0].encoder, None);
    }
}
