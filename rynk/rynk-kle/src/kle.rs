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

use std::ops::Deref;

use serde_json::Value;

/// A placed KLE key and the RMK/Vial meaning carried by its legends.
#[derive(Clone, Debug)]
pub(crate) struct AnnotatedKey {
    pub(crate) key: kle_serial::Key<f64>,
    /// Matrix position from the top-left legend (`None` if it isn't `row,col`, or
    /// if this is an encoder switch).
    pub(crate) matrix: Option<(u8, u8)>,
    /// `(encoder index, rotary action)` when this key is a Vial encoder switch
    /// (marked by a center legend of `e`); action `0` is CCW, `1` is CW.
    pub(crate) encoder: Option<(u8, u8)>,
    /// `group,choice` layout option from any other legend, if present.
    pub(crate) option: Option<(u8, u8)>,
    /// Index of the source keymap row — used to reconstruct RMK map lines.
    pub(crate) row_index: usize,
}

impl Deref for AnnotatedKey {
    type Target = kle_serial::Key<f64>;

    fn deref(&self) -> &Self::Target {
        &self.key
    }
}

/// KLE keys from `kle-serial` together with their RMK/Vial annotations.
#[derive(Clone, Debug)]
pub(crate) struct ParsedKeymap {
    pub(crate) keys: Vec<AnnotatedKey>,
}

impl ParsedKeymap {
    pub(crate) fn has_matrix_or_encoder(&self) -> bool {
        self.keys
            .iter()
            .any(|key| key.matrix.is_some() || key.encoder.is_some())
    }
}

/// A legend of the form `int,int` — the matrix position (top-left) or a
/// `group,choice` layout option (any other slot).
fn parse_rc(label: &str) -> Result<Option<(u8, u8)>, String> {
    let Some((a, b)) = label.trim().split_once(',') else {
        return Ok(None);
    };
    let (a, b) = (a.trim(), b.trim());
    let is_uint = |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    if !is_uint(a) || !is_uint(b) {
        return Ok(None);
    }
    let a = a
        .parse::<u8>()
        .map_err(|_| format!("KLE legend `{label}` exceeds the supported 0..=255 range"))?;
    let b = b
        .parse::<u8>()
        .map_err(|_| format!("KLE legend `{label}` exceeds the supported 0..=255 range"))?;
    Ok(Some((a, b)))
}

/// Parse the KLE `keymap` array into a flat list of placed keys plus annotations.
pub(crate) fn parse_keymap(keymap: &Value) -> Result<ParsedKeymap, String> {
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

    let mut keys = Vec::with_capacity(keyboard.keys.len());
    for (i, key) in keyboard.keys.into_iter().enumerate() {
        // A Vial encoder switch carries a center legend `e`; then the top-left
        // legend is `index,action` (not a matrix cell).
        let top_left = match key.legends[0].as_ref() {
            Some(legend) => parse_rc(&legend.text)?,
            None => None,
        };
        let is_encoder = key
            .legends
            .iter()
            .skip(1)
            .flatten()
            .any(|legend| legend.text.trim() == "e");
        let (matrix, encoder) = if is_encoder { (None, top_left) } else { (top_left, None) };
        let mut option = None;
        for legend in key.legends.iter().skip(1).flatten() {
            if let Some(value) = parse_rc(&legend.text)? {
                option = Some(value);
                break;
            }
        }
        keys.push(AnnotatedKey {
            key,
            matrix,
            encoder,
            option,
            row_index: row_of_key[i],
        });
    }
    Ok(ParsedKeymap { keys })
}

/// Fallback for a plain KLE export, where legends are key labels rather than VIA
/// `row,col` markers: assign matrix positions row-major — row = the key's source
/// KLE row (renumbered densely), col = its order within that row. Decals and
/// encoder switches are left alone.
pub(crate) fn assign_matrix_by_position(keymap: &mut ParsedKeymap) -> Result<(), String> {
    let mut rows: Vec<usize> = Vec::new();
    let mut next_col: Vec<usize> = Vec::new();
    for key in &mut keymap.keys {
        if key.decal || key.encoder.is_some() {
            continue;
        }
        let row = rows.iter().position(|&r| r == key.row_index).unwrap_or_else(|| {
            rows.push(key.row_index);
            next_col.push(0);
            rows.len() - 1
        });
        let matrix_row = u8::try_from(row).map_err(|_| "plain KLE layout has more than 255 matrix rows")?;
        let matrix_col =
            u8::try_from(next_col[row]).map_err(|_| "plain KLE layout has more than 255 matrix columns")?;
        key.matrix = Some((matrix_row, matrix_col));
        next_col[row] += 1;
    }
    Ok(())
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
        assert_eq!(k.keys[0].matrix, Some((0, 0)));
        assert_eq!((k.keys[0].x, k.keys[0].y), (0.0, 0.0));
        assert_eq!((k.keys[1].x, k.keys[1].y), (1.0, 0.0));
        assert_eq!(k.keys[2].matrix, Some((1, 0)));
        assert_eq!((k.keys[2].x, k.keys[2].y), (0.0, 1.0));
        assert_eq!(k.keys[3].row_index, 1);
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
        assert_eq!(k.keys[last].matrix, Some((3, 0)));
        assert_eq!((k.keys[last].x, k.keys[last].y), (4.0, 1.0)); // row3 baseline 3, -2 => 1
    }

    #[test]
    fn layout_option_label_is_parsed() {
        // Top-left is the matrix; a trailing `n,n` legend is group,choice.
        let k = keys_of(json!([["0,13\n\n\n\n\n\n\n\n\n0,1"]]));
        assert_eq!(k.keys[0].matrix, Some((0, 13)));
        assert_eq!(k.keys[0].option, Some((0, 1)));
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
        assert_eq!(k.keys[0].matrix, Some((0, 0)));
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
            k.keys
                .iter()
                .all(|annotation| annotation.matrix.is_none() && annotation.encoder.is_none())
        );
        assign_matrix_by_position(&mut k).unwrap();
        assert_eq!(k.keys[0].matrix, Some((0, 0)));
        assert_eq!(k.keys[2].matrix, Some((0, 2)));
        assert_eq!(k.keys[3].matrix, Some((1, 0))); // Tab
        assert_eq!(k.keys[4].matrix, None); // decal stays unplaced
        assert_eq!(k.keys[5].matrix, Some((1, 1))); // A: the decal takes no column
    }

    #[test]
    fn encoder_switch_is_not_a_matrix_key() {
        // Vial's own encoder syntax: center legend `e`, top-left is `index,action`.
        let k = keys_of(json!([["0,0\n\n\n\n\n\n\n\n\ne", "0,1\n\n\n\n\n\n\n\n\ne"]]));
        assert_eq!(k.keys[0].matrix, None);
        assert_eq!(k.keys[0].encoder, Some((0, 0))); // encoder 0, CCW
        assert_eq!(k.keys[1].encoder, Some((0, 1))); // encoder 0, CW
        // A plain key with the same top-left legend stays a matrix key.
        let plain = keys_of(json!([["0,0"]]));
        assert_eq!(plain.keys[0].matrix, Some((0, 0)));
        assert_eq!(plain.keys[0].encoder, None);
    }
}
