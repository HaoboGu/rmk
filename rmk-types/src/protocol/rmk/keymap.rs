//! Keymap endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::action::KeyAction;

/// Identifies a specific key position in the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct KeyPosition {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

/// Request payload for `SetKeyAction` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetKeyRequest {
    pub position: KeyPosition,
    pub action: KeyAction,
}

// Bulk transfer types live in the `bulk` submodule below and are re-exported
// when the `bulk` feature is enabled. Gating the entire submodule once avoids
// repeating `#[cfg(feature = "bulk")]` on every type, impl, and import.
#[cfg(feature = "bulk")]
mod bulk {
    use heapless::Vec;
    use postcard::experimental::max_size::MaxSize;
    use postcard_schema::Schema;
    use serde::{Deserialize, Serialize};

    use crate::action::KeyAction;
    use crate::constants::BULK_SIZE;

    /// Request payload for `GetKeymapBulk` endpoint.
    ///
    /// Keys are linearized in row-major order starting from `(start_row, start_col)`.
    /// `count` is the number of keys to read; iteration wraps to subsequent
    /// rows when the end of a row is reached.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
    pub struct GetKeymapBulkRequest {
        pub layer: u8,
        pub start_row: u8,
        pub start_col: u8,
        pub count: u8,
    }

    /// Bulk response for getting multiple key actions at once.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
    pub struct GetKeymapBulkResponse {
        pub actions: Vec<KeyAction, BULK_SIZE>,
    }

    impl MaxSize for GetKeymapBulkResponse {
        const POSTCARD_MAX_SIZE: usize = crate::heapless_vec_max_size::<KeyAction, BULK_SIZE>();
    }

    /// Request payload for `SetKeymapBulk` endpoint.
    ///
    /// Keys are linearized in row-major order starting from `(start_row, start_col)`.
    /// Iteration wraps to subsequent rows when the end of a row is reached.
    /// The number of keys to write is derived from `actions.len()`.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
    pub struct SetKeymapBulkRequest {
        pub layer: u8,
        pub start_row: u8,
        pub start_col: u8,
        pub actions: Vec<KeyAction, BULK_SIZE>,
    }

    impl MaxSize for SetKeymapBulkRequest {
        // 3 bytes for layer + start_row + start_col (each `u8::POSTCARD_MAX_SIZE == 1`).
        const POSTCARD_MAX_SIZE: usize = 3 + crate::heapless_vec_max_size::<KeyAction, BULK_SIZE>();
    }
}

#[cfg(feature = "bulk")]
pub use bulk::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::rmk::test_utils::round_trip;

    #[test]
    fn round_trip_key_position() {
        round_trip(&KeyPosition {
            layer: 0,
            row: 5,
            col: 13,
        });
    }

    #[test]
    fn round_trip_set_key_request() {
        round_trip(&SetKeyRequest {
            position: KeyPosition {
                layer: 0,
                row: 0,
                col: 0,
            },
            action: KeyAction::No,
        });
    }

    #[cfg(feature = "bulk")]
    mod bulk {
        use heapless::Vec;

        use super::super::*;
        use crate::action::{Action, KeyAction};
        use crate::constants::BULK_SIZE;
        use crate::keycode::{HidKeyCode, KeyCode};
        use crate::modifier::ModifierCombination;
        use crate::morse::MorseProfile;
        use crate::protocol::rmk::test_utils::{assert_max_size_bound, round_trip};

        /// Largest-encoded `KeyAction` variant: `TapHold` wraps two multi-field
        /// `Action`s and a `MorseProfile(u32)`, many times the size of
        /// `KeyAction::No`. Using it in max-capacity bulk tests makes
        /// `assert_max_size_bound` exercise both the per-element and the
        /// length-prefix dimensions of the bound.
        fn worst_key_action() -> KeyAction {
            let action = Action::KeyWithModifier(KeyCode::Hid(HidKeyCode::A), ModifierCombination::new());
            KeyAction::TapHold(action, action, MorseProfile::const_default())
        }

        #[test]
        fn round_trip_get_keymap_bulk_request() {
            round_trip(&GetKeymapBulkRequest {
                layer: 2,
                start_row: 0,
                start_col: 0,
                count: 32,
            });
        }

        #[test]
        fn round_trip_set_keymap_bulk_request() {
            let mut actions: Vec<KeyAction, BULK_SIZE> = Vec::new();
            actions.push(KeyAction::No).unwrap();
            round_trip(&SetKeymapBulkRequest {
                layer: 0,
                start_row: 0,
                start_col: 0,
                actions,
            });
        }

        #[test]
        fn round_trip_set_keymap_bulk_request_max_capacity() {
            let mut actions: Vec<KeyAction, BULK_SIZE> = Vec::new();
            for _ in 0..BULK_SIZE {
                actions.push(worst_key_action()).unwrap();
            }
            let req = SetKeymapBulkRequest {
                layer: u8::MAX,
                start_row: u8::MAX,
                start_col: u8::MAX,
                actions,
            };
            round_trip(&req);
            assert_max_size_bound(&req);
        }

        #[test]
        fn round_trip_get_keymap_bulk_response_max_capacity() {
            let mut actions: Vec<KeyAction, BULK_SIZE> = Vec::new();
            for _ in 0..BULK_SIZE {
                actions.push(worst_key_action()).unwrap();
            }
            let resp = GetKeymapBulkResponse { actions };
            round_trip(&resp);
            assert_max_size_bound(&resp);
        }
    }
}
