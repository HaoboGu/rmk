//! Combo endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::combo::Combo;

/// Request payload for `SetCombo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetComboRequest {
    pub index: u8,
    pub config: Combo,
}

// ---------------------------------------------------------------------------
// Bulk transfer types
// ---------------------------------------------------------------------------

#[cfg(feature = "bulk")]
use heapless::Vec;

#[cfg(feature = "bulk")]
use crate::constants::BULK_SIZE;

/// Request payload for `GetComboBulk`.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetComboBulkRequest {
    pub start_index: u8,
    pub count: u8,
}

/// Bulk request payload for setting multiple combos at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetComboBulkRequest {
    pub start_index: u8,
    pub configs: Vec<Combo, BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for SetComboBulkRequest {
    const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE + crate::heapless_vec_max_size::<Combo, BULK_SIZE>();
}

/// Bulk response for getting multiple combos at once.
#[cfg(feature = "bulk")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct GetComboBulkResponse {
    pub configs: Vec<Combo, BULK_SIZE>,
}

#[cfg(feature = "bulk")]
impl MaxSize for GetComboBulkResponse {
    const POSTCARD_MAX_SIZE: usize = crate::heapless_vec_max_size::<Combo, BULK_SIZE>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::KeyAction;
    use crate::constants::COMBO_SIZE;
    use crate::protocol::rmk::test_utils::{assert_max_size_bound, round_trip};

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

    #[cfg(feature = "bulk")]
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

    #[cfg(feature = "bulk")]
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
