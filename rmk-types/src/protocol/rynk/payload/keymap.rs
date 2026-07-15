//! Keymap endpoint types.

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::action::KeyAction;

/// Identifies a specific key position in the keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct KeyPosition {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

/// Request payload for `SetKeyAction` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SetKeyRequest {
    pub position: KeyPosition,
    pub action: KeyAction,
}

mod bulk {
    use postcard::experimental::max_size::MaxSize;
    use serde::{Deserialize, Serialize};

    use crate::action::KeyAction;
    #[cfg(not(feature = "host"))]
    use crate::constants::BULK_KEYMAP_SIZE;

    // Firmware uses a bounded Vec; host bounds transfers from capabilities.
    #[cfg(not(feature = "host"))]
    type BulkActions = heapless::Vec<KeyAction, BULK_KEYMAP_SIZE>;
    #[cfg(feature = "host")]
    type BulkActions = alloc::vec::Vec<KeyAction>;

    /// Request payload for `GetKeymapBulk` endpoint.
    ///
    /// The run starts at key `(layer, start_row, start_col)` and reads forward
    /// through the flat, row-major, layer-major keymap — crossing row and layer
    /// boundaries freely. The firmware returns as many consecutive keys as fit
    /// (`max_bulk_keys`), or fewer at the end of the keymap.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
    #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct GetKeymapBulkRequest {
        pub layer: u8,
        pub start_row: u8,
        pub start_col: u8,
    }

    /// Bulk response for getting multiple key actions at once.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct GetKeymapBulkResponse {
        #[cfg_attr(feature = "wasm", tsify(type = "KeyAction[]"))]
        pub actions: BulkActions,
    }

    // Firmware sizes its fixed buffer from this exact bound; host builds leave
    // the field unbounded and never need `MaxSize`.
    #[cfg(not(feature = "host"))]
    impl MaxSize for GetKeymapBulkResponse {
        const POSTCARD_MAX_SIZE: usize = crate::heapless_vec_max_size::<KeyAction, BULK_KEYMAP_SIZE>();
    }

    impl GetKeymapBulkResponse {
        /// Build the response, collecting up to the bulk capacity.
        pub fn from_iter_bounded(actions: impl IntoIterator<Item = KeyAction>) -> Self {
            #[cfg(not(feature = "host"))]
            let actions = actions.into_iter().take(BULK_KEYMAP_SIZE).collect();
            #[cfg(feature = "host")]
            let actions = actions.into_iter().collect();
            Self { actions }
        }
    }

    /// Request payload for `SetKeymapBulk` endpoint.
    ///
    /// Writes `actions` into the flat, row-major, layer-major keymap starting at
    /// key `(layer, start_row, start_col)`, continuing across rows and layers.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct SetKeymapBulkRequest {
        pub layer: u8,
        pub start_row: u8,
        pub start_col: u8,
        #[cfg_attr(feature = "wasm", tsify(type = "KeyAction[]"))]
        pub actions: BulkActions,
    }

    #[cfg(not(feature = "host"))]
    impl MaxSize for SetKeymapBulkRequest {
        // layer + start_row + start_col, then the bounded actions vector.
        const POSTCARD_MAX_SIZE: usize =
            3 * <u8 as MaxSize>::POSTCARD_MAX_SIZE + crate::heapless_vec_max_size::<KeyAction, BULK_KEYMAP_SIZE>();
    }
}

pub use bulk::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::rynk::tests::round_trip;

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

    // Firmware-only: exercises heapless keymap bulk capacity.
    #[cfg(not(feature = "host"))]
    mod bulk {
        use heapless::Vec;

        use super::super::*;
        use crate::action::{Action, KeyAction};
        use crate::constants::BULK_KEYMAP_SIZE;
        use crate::keycode::HidKeyCode;
        use crate::modifier::ModifierCombination;
        use crate::protocol::rynk::tests::{assert_max_size_bound, round_trip};

        /// Largest-encoded `KeyAction` variant: `TapHold` wraps two multi-field
        /// `Action`s (plus a `u8` profile index, worst-case `u8::MAX`), many
        /// times the size of `KeyAction::No`. Using it in max-capacity bulk
        /// tests makes `assert_max_size_bound` exercise both the per-element
        /// and the length-prefix dimensions of the bound.
        fn worst_key_action() -> KeyAction {
            let action = Action::KeyWithModifier(HidKeyCode::A, ModifierCombination::new());
            KeyAction::TapHold(action, action, u8::MAX)
        }

        #[test]
        fn round_trip_get_keymap_bulk_request() {
            round_trip(&GetKeymapBulkRequest {
                layer: 2,
                start_row: 0,
                start_col: 0,
            });
        }

        #[test]
        fn round_trip_set_keymap_bulk_request() {
            let mut actions: Vec<KeyAction, BULK_KEYMAP_SIZE> = Vec::new();
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
            let mut actions: Vec<KeyAction, BULK_KEYMAP_SIZE> = Vec::new();
            for _ in 0..BULK_KEYMAP_SIZE {
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
            let mut actions: Vec<KeyAction, BULK_KEYMAP_SIZE> = Vec::new();
            for _ in 0..BULK_KEYMAP_SIZE {
                actions.push(worst_key_action()).unwrap();
            }
            let resp = GetKeymapBulkResponse { actions };
            round_trip(&resp);
            assert_max_size_bound(&resp);
        }
    }
}
