use num_enum::FromPrimitive;

use crate::{
    keycode::KeyCode,
    keymap::fill_vec,
    via::keycode_convert::{from_ascii, to_ascii},
    MACRO_SPACE_SIZE,
};

/// encoded with the two bytes, content at the third byte
/// 0b 0000 0001 1000-1010 (VIAL_MACRO_EXT) are not supported
///
/// TODO save space: refacter to use 1 byte for encoding and convert to/from vial 2 byte encoding
#[derive(Debug, Clone)]
pub enum MacroOperation {
    /// 0x00, 1 byte
    /// Marks the end of a macro sequence
    /// Don't use it on your own,
    /// will be automatically removed and added
    /// by MacroOperations::define_macro_sequences()
    End,
    /// 0x01 01 + 1 byte keycode
    Tap(KeyCode),
    /// 0x01 02 + 1 byte keycode
    Press(KeyCode),
    /// 0x01 03 + 1 byte keycode
    Release(KeyCode),
    /// 0x01 04 + 2 byte for the delay in ms
    Delay(u16),
    /// Anything not covered above (and starting at
    /// 0x30 (= b'0'), is the 1 byte ascii character.
    Text(KeyCode, bool), // bool = shifted
}

impl MacroOperation {
    /// Get the next macro operation starting from given index and offset (=position in the sequence)
    /// Return current macro operation and the next operations's offset
    pub(crate) fn get_next_macro_operation(
        macro_sequences: &[u8],
        macro_start_idx: usize,
        offset: usize,
    ) -> (MacroOperation, usize) {
        let idx = macro_start_idx + offset;
        if idx >= macro_sequences.len() - 1 {
            return (MacroOperation::End, offset);
        }
        match (macro_sequences[idx], macro_sequences[idx + 1]) {
            (0, _) => (MacroOperation::End, offset),
            (1, 1) => {
                // SS_QMK_PREFIX + SS_TAP_CODE
                if idx + 2 < macro_sequences.len() {
                    let keycode = KeyCode::from_primitive(macro_sequences[idx + 2] as u16);
                    (MacroOperation::Tap(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 2) => {
                // SS_QMK_PREFIX + SS_DOWN_CODE
                if idx + 2 < macro_sequences.len() {
                    let keycode = KeyCode::from_primitive(macro_sequences[idx + 2] as u16);
                    (MacroOperation::Press(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 3) => {
                // SS_QMK_PREFIX + SS_UP_CODE
                if idx + 2 < macro_sequences.len() {
                    let keycode = KeyCode::from_primitive(macro_sequences[idx + 2] as u16);
                    (MacroOperation::Release(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 4) => {
                // SS_QMK_PREFIX + SS_DELAY_CODE
                if idx + 3 < macro_sequences.len() {
                    let delay_ms = (macro_sequences[idx + 2] as u16 - 1) + (macro_sequences[idx + 3] as u16 - 1) * 255;
                    (MacroOperation::Delay(delay_ms), offset + 4)
                } else {
                    (MacroOperation::End, offset + 4)
                }
            }
            (1, 5) | (1, 6) | (1, 7) => {
                warn!("VIAL_MACRO_EXT is not supported");
                (MacroOperation::Delay(0), offset + 4)
            }
            _ => {
                // Current byte is the ascii code, convert it to keyboard keycode(with caps state)
                let (keycode, is_caps) = from_ascii(macro_sequences[idx]);
                (MacroOperation::Text(keycode, is_caps), offset + 1)
            }
        }
    }

    /// finds the start of a macro sequence by providing a guessed start index
    pub(crate) fn get_macro_sequence_start(macro_sequences: &[u8], guessed_macro_start_idx: u8) -> Option<usize> {
        let mut idx = 0;
        // Find idx until the macro start of given index
        let mut potential_start_idx = guessed_macro_start_idx;
        loop {
            if potential_start_idx == 0 || idx >= macro_sequences.len() {
                break;
            }
            if macro_sequences[idx] == 0 {
                potential_start_idx -= 1;
            }
            idx += 1;
        }

        if idx == macro_sequences.len() {
            None
        } else {
            Some(idx)
        }
    }
}

/// serializes macro sequences
/// macros are filled up with 0 if shorter than MACRO_SPACE_SIZE
/// so that it has enough space for macros defined my Vial
/// panics if the resulting binary macro sequence is longer than MACRO_SPACE_SIZE
pub fn define_macro_sequences(
    macro_sequences: &[heapless::Vec<MacroOperation, MACRO_SPACE_SIZE>],
) -> [u8; MACRO_SPACE_SIZE] {
    // TODO after binary format is understood and
    // TEXT is smaller than others,
    // refactor, exchanging tab for text (as this is shorter),
    // taking care of press/release LSHIFT and RSHIFT as well
    let mut macro_sequences_linear = fold_to_binary(macro_sequences);

    fill_vec(&mut macro_sequences_linear);
    macro_sequences_linear
        .into_array()
        .expect("as we resized the vector, this can't happen!")
}

/// Convinience function to convert a String into a sequence of MacroOptions::Text.
/// Currently ponly u8 ascii is supported.
pub fn to_macro_sequence(text: &str) -> heapless::Vec<MacroOperation, MACRO_SPACE_SIZE> {
    // if !text.is_ascii() {
    //     compile_error!("Only ascii text is supported!")
    // };
    text.as_bytes()
        .iter()
        .map(|character| {
            let (keycode, shifted) = from_ascii(*character);
            MacroOperation::Text(keycode, shifted)
        })
        .collect()
}

/// converts macro sequences [Vec<MacroOperation] binary form and flattens to Vec<u8, MACRO_SPACE_SIZE>
/// Note that the Vec is still at it's minimal needed length and needs to be etended with zeros to the desired size
/// (with vec.resize())
fn fold_to_binary(
    macro_sequences: &[heapless::Vec<MacroOperation, MACRO_SPACE_SIZE>],
) -> heapless::Vec<u8, MACRO_SPACE_SIZE> {
    // TODO after binary format is understood and
    // TEXT is smaller than others,
    // refactor, exchanging tab for text (as this is shorter),
    // taking care of press/release LSHIFT and RSHIFT as well
    const TOO_MANY_ELEMENTS_ERROR_TEXT: &str = "Too many Macro Operations! The sum of all Macro Operations of all Macro Sequences cannot be more than MACRO_SPACE_SIZE";

    macro_sequences
        .iter()
        .map(|macro_sequence| {
            let mut vec_seq = macro_sequence
                .into_iter()
                .filter(|macro_operation| !matches!(macro_operation, MacroOperation::End))
                .map(serialize)
                .fold(heapless::Vec::<u8, MACRO_SPACE_SIZE>::new(), |mut acc, e| {
                    acc.extend_from_slice(&e).expect(TOO_MANY_ELEMENTS_ERROR_TEXT);
                    acc
                });
            vec_seq.push(0x00).expect(TOO_MANY_ELEMENTS_ERROR_TEXT); //= serialize(&MacroOperation::End));
            vec_seq
        })
        .fold(heapless::Vec::<u8, MACRO_SPACE_SIZE>::new(), |mut acc, e| {
            acc.extend_from_slice(&e).expect(TOO_MANY_ELEMENTS_ERROR_TEXT);
            acc
        })
}

fn serialize(macro_operation: &MacroOperation) -> heapless::Vec<u8, 4> {
    match macro_operation {
        MacroOperation::End => heapless::Vec::from_slice(&[0x00]).unwrap(),
        MacroOperation::Tap(key_code) => {
            let mut result = heapless::Vec::from_slice(&[0x01, 0x01]).unwrap();
            // TODO check is Keycode is correct
            result
                .extend_from_slice(&[(*key_code as u16).to_be_bytes()[1]])
                .expect("impossible error");
            result
        }
        MacroOperation::Press(key_code) => {
            let mut result = heapless::Vec::from_slice(&[0x01, 0x02]).unwrap();
            // TODO check is Keycode is correct
            result
                .extend_from_slice(&[(*key_code as u16).to_be_bytes()[1]])
                .expect("impossible error");
            result
        }
        MacroOperation::Release(key_code) => {
            let mut result = heapless::Vec::from_slice(&[0x01, 0x03]).unwrap();
            result
                .extend_from_slice(&[(*key_code as u16).to_be_bytes()[1]])
                .expect("impossible error");
            result
        }
        MacroOperation::Delay(duration) => {
            let mut result = heapless::Vec::from_slice(&[0x01, 0x04]).unwrap();
            result
                .extend_from_slice(&duration.to_be_bytes())
                .expect("impossible error");
            result
        }
        MacroOperation::Text(key_code, shifted) => heapless::Vec::from_slice(&[to_ascii(*key_code, *shifted)]).unwrap(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_define_one_macro_sequence_manual() {
        let macro_sequences = &[heapless::Vec::from_slice(&[
            MacroOperation::Press(KeyCode::LShift),
            MacroOperation::Tap(KeyCode::P),
            MacroOperation::Release(KeyCode::LShift),
            MacroOperation::Tap(KeyCode::A),
            MacroOperation::Tap(KeyCode::T),
        ])
        .expect("too many elements")];
        let macro_sequences_binary = define_macro_sequences(macro_sequences);
        // let result = [0b 0000 0000 0100 1000]
        let result: [u8; 16] = [
            0x01, 0x02, 0xE1, 0x01, 0x01, 0x13, 0x01, 0x03, 0xE1, 0x01, 0x01, 0x4, 0x01, 0x01, 0x17, 0x00,
        ];
        let mut result_filled = [0; MACRO_SPACE_SIZE];
        for (i, element) in result.into_iter().enumerate() {
            result_filled[i] = element
        }
        assert_eq!(macro_sequences_binary, result_filled);
    }
    #[test]
    fn test_define_two_macro_sequence_manual() {
        let macro_sequences_terminated_uneccessarily = [
            heapless::Vec::from_slice(&[
                MacroOperation::Text(KeyCode::H, true),
                MacroOperation::Text(KeyCode::I, false),
            ])
            .expect("too many elements"),
            heapless::Vec::from_slice(&[
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::P),
                MacroOperation::Release(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::A),
                MacroOperation::Tap(KeyCode::T),
            ])
            .expect("too many elements"),
        ];
        let macro_sequences_binary = define_macro_sequences(&macro_sequences_terminated_uneccessarily);
        let result: [u8; 19] = [
            0x48, 0x69, 0x00, 0x01, 0x02, 0xE1, 0x01, 0x01, 0x13, 0x01, 0x03, 0xE1, 0x01, 0x01, 0x4, 0x01, 0x01, 0x17,
            0x00,
        ];
        let mut result_filled = [0; MACRO_SPACE_SIZE];
        for (i, element) in result.into_iter().enumerate() {
            result_filled[i] = element
        }
        assert_eq!(macro_sequences_binary, result_filled);
    }

    #[test]
    fn test_define_macro_sequences_clean() {
        let macro_sequences_clean = [
            heapless::Vec::from_slice(&[
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::H),
                MacroOperation::Release(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::E),
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::Tap(KeyCode::O),
            ])
            .expect("too many elements"),
            heapless::Vec::from_slice(&[
                MacroOperation::Tap(KeyCode::W),
                MacroOperation::Tap(KeyCode::O),
                MacroOperation::Tap(KeyCode::R),
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::Tap(KeyCode::D),
            ])
            .expect("too many elements"),
            heapless::Vec::from_slice(&[
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::Kc2),
                MacroOperation::Release(KeyCode::LShift),
            ])
            .expect("too many elements"),
        ];
        let macro_sequences_binary = define_macro_sequences(&macro_sequences_clean);
        let result: [u8; 48] = [
            1, 2, 225, 1, 1, 11, 1, 3, 225, 1, 1, 8, 1, 1, 15, 1, 1, 15, 1, 1, 18, 0, 1, 1, 26, 1, 1, 18, 1, 1, 21, 1,
            1, 15, 1, 1, 7, 0, 1, 2, 225, 1, 1, 31, 1, 3, 225, 0,
        ];
        let mut result_filled = [0; MACRO_SPACE_SIZE];
        for (i, element) in result.into_iter().enumerate() {
            result_filled[i] = element
        }
        assert_eq!(macro_sequences_binary, result_filled);
    }

    #[test]
    fn test_define_macro_sequences_uneccessarily_terminated() {
        let macro_sequences_terminated_uneccessarily = [
            heapless::Vec::from_slice(&[
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::H),
                MacroOperation::Release(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::E),
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::Tap(KeyCode::O),
                MacroOperation::End,
            ])
            .expect("too many elements"),
            heapless::Vec::from_slice(&[
                MacroOperation::Tap(KeyCode::W),
                MacroOperation::Tap(KeyCode::O),
                MacroOperation::Tap(KeyCode::R),
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::End,
            ])
            .expect("too many elements"),
            heapless::Vec::from_slice(&[
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::Kc2),
                MacroOperation::Release(KeyCode::LShift),
                MacroOperation::End,
            ])
            .expect("too many elements"),
        ];
        let macro_sequences_binary = define_macro_sequences(&macro_sequences_terminated_uneccessarily);
        let result: [u8; 45] = [
            1, 2, 225, 1, 1, 11, 1, 3, 225, 1, 1, 8, 1, 1, 15, 1, 1, 15, 1, 1, 18, 0, 1, 1, 26, 1, 1, 18, 1, 1, 21, 1,
            1, 15, 0, 1, 2, 225, 1, 1, 31, 1, 3, 225, 0,
        ];
        let mut result_filled = [0; MACRO_SPACE_SIZE];
        for (i, element) in result.into_iter().enumerate() {
            result_filled[i] = element
        }
        assert_eq!(macro_sequences_binary, result_filled);
    }

    #[test]
    fn test_define_macro_sequences_random_end_markers() {
        let macro_sequences_random_end_markers = [
            heapless::Vec::from_slice(&[
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::H),
                MacroOperation::End,
                MacroOperation::Release(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::E),
                MacroOperation::End,
                MacroOperation::End,
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::End,
                MacroOperation::Tap(KeyCode::L),
                MacroOperation::Tap(KeyCode::O),
                MacroOperation::End,
            ])
            .expect("too many elements"),
            heapless::Vec::from_slice(&[
                MacroOperation::Tap(KeyCode::W),
                MacroOperation::Tap(KeyCode::O),
                MacroOperation::End,
                MacroOperation::End,
                MacroOperation::End,
                MacroOperation::End,
                MacroOperation::Tap(KeyCode::R),
                MacroOperation::Tap(KeyCode::L),
            ])
            .expect("too many elements"),
            heapless::Vec::from_slice(&[
                MacroOperation::Press(KeyCode::LShift),
                MacroOperation::Tap(KeyCode::Kc2),
                MacroOperation::Release(KeyCode::LShift),
                MacroOperation::End,
                MacroOperation::End,
                MacroOperation::End,
                MacroOperation::End,
                MacroOperation::End,
            ])
            .expect("too many elements"),
        ];
        let macro_sequences_binary = define_macro_sequences(&macro_sequences_random_end_markers);
        let result: [u8; 45] = [
            1, 2, 225, 1, 1, 11, 1, 3, 225, 1, 1, 8, 1, 1, 15, 1, 1, 15, 1, 1, 18, 0, 1, 1, 26, 1, 1, 18, 1, 1, 21, 1,
            1, 15, 0, 1, 2, 225, 1, 1, 31, 1, 3, 225, 0,
        ];
        let mut result_filled = [0; MACRO_SPACE_SIZE];
        for (i, element) in result.into_iter().enumerate() {
            result_filled[i] = element
        }
        assert_eq!(macro_sequences_binary, result_filled);
    }
}
