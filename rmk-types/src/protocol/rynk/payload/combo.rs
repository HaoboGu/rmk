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

mod bulk {
    use postcard::experimental::max_size::MaxSize;
    use serde::{Deserialize, Serialize};

    use crate::combo::Combo;
    #[cfg(not(feature = "host"))]
    use crate::constants::BULK_SIZE;

    // Firmware uses a bounded Vec; host bounds transfers from capabilities.
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

    // Firmware sizes its fixed buffer from these exact bounds; host builds leave
    // the fields unbounded and never need `MaxSize`.
    #[cfg(not(feature = "host"))]
    impl MaxSize for SetComboBulkRequest {
        // start_index, then the bounded configs vector.
        const POSTCARD_MAX_SIZE: usize =
            <u8 as MaxSize>::POSTCARD_MAX_SIZE + crate::heapless_vec_max_size::<Combo, BULK_SIZE>();
    }

    /// Bulk response for getting multiple combos at once.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct GetComboBulkResponse {
        #[cfg_attr(feature = "wasm", tsify(type = "Combo[]"))]
        pub configs: BulkCombos,
    }

    #[cfg(not(feature = "host"))]
    impl MaxSize for GetComboBulkResponse {
        const POSTCARD_MAX_SIZE: usize = crate::heapless_vec_max_size::<Combo, BULK_SIZE>();
    }

    impl GetComboBulkResponse {
        /// Build the response, collecting up to the bulk capacity.
        pub fn from_iter_bounded(configs: impl IntoIterator<Item = Combo>) -> Self {
            #[cfg(not(feature = "host"))]
            let configs = configs.into_iter().take(BULK_SIZE).collect();
            #[cfg(feature = "host")]
            let configs = configs.into_iter().collect();
            Self { configs }
        }
    }
}

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

    // Firmware-only: exercises heapless bulk capacity.
    #[cfg(not(feature = "host"))]
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
