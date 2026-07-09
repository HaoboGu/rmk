//! Combo endpoint types.

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::combo::Combo;

/// Request payload for `SetCombo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SetComboRequest {
    pub index: u8,
    pub config: Combo,
}

// Bulk transfer types live in the `bulk` submodule below and are re-exported
// when the `bulk` feature is enabled. Gating the entire submodule once avoids
// repeating `#[cfg(feature = "bulk")]` on every type, impl, and import.
#[cfg(feature = "bulk")]
mod bulk {
    use postcard::experimental::max_size::MaxSize;
    use serde::{Deserialize, Serialize};

    use crate::combo::Combo;
    #[cfg(not(feature = "host"))]
    use crate::constants::BULK_SIZE;

    // Firmware sizes this Vec from the buffer (`BULK_SIZE`, derived from
    // `RYNK_BUFFER_SIZE`); the host leaves it unbounded (`alloc::Vec`) and bounds
    // each transfer at runtime from the firmware-reported `max_bulk_configs`.
    #[cfg(not(feature = "host"))]
    type BulkCombos = heapless::Vec<Combo, BULK_SIZE>;
    #[cfg(feature = "host")]
    type BulkCombos = alloc::vec::Vec<Combo>;

    /// Request payload for `GetComboBulk`: read a page of combos starting at slot
    /// `start_index`. The firmware returns as many as fit (`max_bulk_configs`),
    /// fewer at the end, or an empty page once `start_index` reaches the slot count.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
    #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct GetComboBulkRequest {
        pub start_index: u8,
    }

    /// Bulk request payload for `SetComboBulk`: write `configs` starting at slot
    /// `start_index`.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct SetComboBulkRequest {
        pub start_index: u8,
        #[cfg_attr(feature = "wasm", tsify(type = "Combo[]"))]
        pub configs: BulkCombos,
    }

    // `@bulk` endpoints are excluded from the buffer-floor fold, so this `MaxSize`
    // is decorative — it only satisfies the `Endpoint: MaxSize` bound. A bulk frame
    // fits the buffer by construction, so the buffer size is a valid (if loose)
    // bound that host and firmware can share.
    impl MaxSize for SetComboBulkRequest {
        const POSTCARD_MAX_SIZE: usize = crate::constants::RYNK_BUFFER_SIZE;
    }

    /// Bulk response for getting multiple combos at once.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct GetComboBulkResponse {
        #[cfg_attr(feature = "wasm", tsify(type = "Combo[]"))]
        pub configs: BulkCombos,
    }

    impl MaxSize for GetComboBulkResponse {
        const POSTCARD_MAX_SIZE: usize = crate::constants::RYNK_BUFFER_SIZE;
    }

    impl GetComboBulkResponse {
        /// Build the response, collecting up to the bulk capacity.
        #[cfg(not(feature = "host"))]
        pub fn from_iter_bounded(configs: impl IntoIterator<Item = Combo>) -> Self {
            let mut v = BulkCombos::new();
            for c in configs {
                if v.push(c).is_err() {
                    break;
                }
            }
            Self { configs: v }
        }
        #[cfg(feature = "host")]
        pub fn from_iter_bounded(configs: impl IntoIterator<Item = Combo>) -> Self {
            Self {
                configs: configs.into_iter().collect(),
            }
        }
    }
}

#[cfg(feature = "bulk")]
pub use bulk::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::KeyAction;
    use crate::constants::COMBO_SIZE;
    use crate::protocol::rynk::tests::{assert_max_size_bound, round_trip};

    /// Build a `Combo` filled to `COMBO_SIZE` actions plus a `Some` layer —
    /// the worst case for the manual `MaxSize` impl on `Combo`.
    fn full_combo() -> Combo {
        let actions = core::iter::repeat_n(
            KeyAction::Single(crate::action::Action::Key(crate::keycode::KeyCode::Hid(
                crate::keycode::HidKeyCode::A,
            ))),
            COMBO_SIZE,
        );
        Combo::new(actions, KeyAction::No, Some(u8::MAX))
    }

    #[test]
    fn round_trip_combo() {
        round_trip(&Combo::new([KeyAction::No], KeyAction::No, Some(1)));
        round_trip(&Combo::empty());
    }

    #[test]
    fn round_trip_set_combo_request() {
        round_trip(&SetComboRequest {
            index: 3,
            config: Combo::new([KeyAction::No], KeyAction::No, Some(1)),
        });
    }

    #[test]
    fn round_trip_combo_max_capacity() {
        let c = full_combo();
        assert_eq!(c.actions.len(), COMBO_SIZE);
        round_trip(&c);
        assert_max_size_bound(&c);
    }

    // Firmware-only: exercises the heapless capacity (`BULK_SIZE`). On the host the
    // field is an unbounded `alloc::Vec`; the wire format is identical and pinned by
    // the snapshot tests.
    #[cfg(all(feature = "bulk", not(feature = "host")))]
    mod bulk {
        use heapless::Vec;

        use super::super::*;
        use super::full_combo;
        use crate::combo::Combo;
        use crate::constants::BULK_SIZE;
        use crate::protocol::rynk::tests::{assert_max_size_bound, round_trip};

        #[test]
        fn round_trip_set_combo_bulk_request_max_capacity() {
            let mut configs: Vec<Combo, BULK_SIZE> = Vec::new();
            for _ in 0..BULK_SIZE {
                configs.push(full_combo()).unwrap();
            }
            let req = SetComboBulkRequest {
                start_index: u8::MAX,
                configs,
            };
            round_trip(&req);
            assert_max_size_bound(&req);
        }

        #[test]
        fn round_trip_get_combo_bulk_response_max_capacity() {
            let mut configs: Vec<Combo, BULK_SIZE> = Vec::new();
            for _ in 0..BULK_SIZE {
                configs.push(full_combo()).unwrap();
            }
            let resp = GetComboBulkResponse { configs };
            round_trip(&resp);
            assert_max_size_bound(&resp);
        }
    }
}
