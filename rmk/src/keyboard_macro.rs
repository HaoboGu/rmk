use num_enum::FromPrimitive;

use crate::{keycode::KeyCode, via::keycode_convert::from_ascii};

/// Default macro space size
/// the sum of alll macro elements + number of macro elements
pub(crate) const MACRO_SPACE_SIZE: usize = 256;

/// Default number of keyboard macros
pub(crate) const NUM_MACRO: usize = 8;

/// encoded with the two bytes, content at the third byte
/// 0b 0000 0001 1000-1010 (VIAL_MACRO_EXT) are not supported
///
/// TODO save space: refacter to use 1 byte for encoding and convert to/from vial 2 byte encoding
#[derive(Clone)]
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
        macro_cache: &[u8],
        macro_start_idx: usize,
        offset: usize,
    ) -> (MacroOperation, usize) {
        let idx = macro_start_idx + offset;
        if idx >= macro_cache.len() - 1 {
            return (MacroOperation::End, offset);
        }
        match (macro_cache[idx], macro_cache[idx + 1]) {
            (0, _) => (MacroOperation::End, offset),
            (1, 1) => {
                // SS_QMK_PREFIX + SS_TAP_CODE
                if idx + 2 < macro_cache.len() {
                    let keycode = KeyCode::from_primitive(macro_cache[idx + 2] as u16);
                    (MacroOperation::Tap(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 2) => {
                // SS_QMK_PREFIX + SS_DOWN_CODE
                if idx + 2 < macro_cache.len() {
                    let keycode = KeyCode::from_primitive(macro_cache[idx + 2] as u16);
                    (MacroOperation::Press(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 3) => {
                // SS_QMK_PREFIX + SS_UP_CODE
                if idx + 2 < macro_cache.len() {
                    let keycode = KeyCode::from_primitive(macro_cache[idx + 2] as u16);
                    (MacroOperation::Release(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 4) => {
                // SS_QMK_PREFIX + SS_DELAY_CODE
                if idx + 3 < macro_cache.len() {
                    let delay_ms =
                        (macro_cache[idx + 2] as u16 - 1) + (macro_cache[idx + 3] as u16 - 1) * 255;
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
                let (keycode, is_caps) = from_ascii(macro_cache[idx]);
                (MacroOperation::Text(keycode, is_caps), offset + 1)
            }
        }
    }

    /// finds the start of a macro sequence by providing a guessed start index
    pub(crate) fn get_macro_sequence_start(
        macro_cache: &[u8],
        guessed_macro_start_idx: u8,
    ) -> Option<usize> {
        let mut idx = 0;
        // Find idx until the macro start of given index
        let mut potential_start_idx = guessed_macro_start_idx;
        loop {
            if potential_start_idx == 0 || idx >= macro_cache.len() {
                break;
            }
            if macro_cache[idx] == 0 {
                potential_start_idx -= 1;
            }
            idx += 1;
        }

        if idx == macro_cache.len() {
            None
        } else {
            Some(idx)
        }
    }
}

// /// serializes macro sequences
// pub fn define_macro_sequences<const N: usize>(
//     macro_sequences: &[heapless::Vec<MacroOperation, N>],
// ) -> [u8; MACRO_SPACE_SIZE] {
//     let macro_sequences = [
//         heapless::Vec::from_slice(&[
//             MacroOperation::Press(KeyCode::LShift),
//             MacroOperation::Tap(KeyCode::H),
//             MacroOperation::Release(KeyCode::LShift),
//             MacroOperation::Tap(KeyCode::E),
//             MacroOperation::Tap(KeyCode::L),
//             MacroOperation::Tap(KeyCode::L),
//             MacroOperation::Tap(KeyCode::O),
//         ])
//         .expect("too many elements"),
//         heapless::Vec::from_slice(&[
//             MacroOperation::Tap(KeyCode::W),
//             MacroOperation::Tap(KeyCode::O),
//             MacroOperation::Tap(KeyCode::R),
//             MacroOperation::Tap(KeyCode::L),
//             MacroOperation::Tap(KeyCode::D),
//             MacroOperation::End,
//         ])
//         .expect("too many elements"),
//         heapless::Vec::from_slice(&[
//             MacroOperation::Press(KeyCode::LShift),
//             MacroOperation::Tap(KeyCode::Kc2),
//             MacroOperation::Release(KeyCode::LShift),
//         ])
//         .expect("too many elements"),
//     ];

//     // if macro_sequences
//     //     .iter()
//     //     .map(|macro_sequence| macro_sequence.len())
//     //     .reduce(|acc, e| acc + e)
//     //     .expect("error converting to len")
//     //     > MACRO_SPACE_SIZE
//     // {
//     //     compile_error!("More macro elements than MACRO_SPACE_SIZE");
//     // }

//     let macro_cache = heapless::Vec::<MacroOperation, MACRO_SPACE_SIZE>::new();

//     // serialize - look into on-the-fly deserialization
//     for macro_sequence in macro_sequences {
//         for macro_element in macro_sequence {
//             if let MacroOperation::End = macro_element {
//                 continue;
//             }
//             macro_cache.push(macro_element);
//         }
//         macro_cache.push(MacroOperation::End);
//     }
//     // check again if len is ok, as we removed MacroOperation::End and added for each sequence
//     if macro_cache.len() > MACRO_SPACE_SIZE {
//         compile_error!("More macro elements than MACRO_SPACE_SIZE");
//     }

//     macro_cache
//         .into_array()
//         .expect("could not convert reslt array")
// }
