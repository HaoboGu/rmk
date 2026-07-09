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

// Bulk transfer types live in the `bulk` submodule below and are re-exported
// when the `bulk` feature is enabled. Gating the entire submodule once avoids
// repeating `#[cfg(feature = "bulk")]` on every type, impl, and import.
#[cfg(feature = "bulk")]
mod bulk {
    use postcard::experimental::max_size::MaxSize;
    use serde::{Deserialize, Serialize};

    use crate::action::KeyAction;
    #[cfg(not(feature = "host"))]
    use crate::constants::BULK_KEYMAP_SIZE;

    // Firmware sizes this Vec from the buffer (`BULK_KEYMAP_SIZE`, derived from
    // `RYNK_BUFFER_SIZE`); the host leaves it unbounded (`alloc::Vec`) and bounds
    // each transfer at runtime from the firmware-reported `max_bulk_keys`.
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

    // `@bulk` endpoints are excluded from the buffer-floor fold, so this `MaxSize`
    // is decorative — it only satisfies the `Endpoint: MaxSize` bound. A bulk frame
    // fits the buffer by construction, so the buffer size is a valid (if loose)
    // bound that host and firmware can share.
    impl MaxSize for GetKeymapBulkResponse {
        const POSTCARD_MAX_SIZE: usize = crate::constants::RYNK_BUFFER_SIZE;
    }

    impl GetKeymapBulkResponse {
        /// Build the response, collecting up to the bulk capacity.
        #[cfg(not(feature = "host"))]
        pub fn from_iter_bounded(actions: impl IntoIterator<Item = KeyAction>) -> Self {
            let mut v = BulkActions::new();
            for a in actions {
                if v.push(a).is_err() {
                    break;
                }
            }
            Self { actions: v }
        }
        #[cfg(feature = "host")]
        pub fn from_iter_bounded(actions: impl IntoIterator<Item = KeyAction>) -> Self {
            Self {
                actions: actions.into_iter().collect(),
            }
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

    impl MaxSize for SetKeymapBulkRequest {
        const POSTCARD_MAX_SIZE: usize = crate::constants::RYNK_BUFFER_SIZE;
    }
}

#[cfg(feature = "bulk")]
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

    // Firmware-only: these exercise the heapless capacity (`BULK_KEYMAP_SIZE`). On
    // the host the field is an unbounded `alloc::Vec`, so a "max capacity" test is
    // not meaningful; the wire format is identical and pinned by the snapshot tests.
    #[cfg(all(feature = "bulk", not(feature = "host")))]
    mod bulk {
        use heapless::Vec;

        use super::super::*;
        use crate::action::{Action, KeyAction};
        use crate::constants::BULK_KEYMAP_SIZE;
        use crate::keycode::{HidKeyCode, KeyCode};
        use crate::modifier::ModifierCombination;
        use crate::morse::MorseProfile;
        use crate::protocol::rynk::tests::{assert_max_size_bound, round_trip};

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
