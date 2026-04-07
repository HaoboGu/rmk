//! Morse endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::morse::Morse;

/// Request payload for `SetMorse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetMorseRequest {
    pub index: u8,
    pub config: Morse,
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

    use crate::constants::BULK_SIZE;
    use crate::morse::Morse;

    /// Request payload for `GetMorseBulk`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
    pub struct GetMorseBulkRequest {
        pub start_index: u8,
        pub count: u8,
    }

    /// Bulk request payload for setting multiple morse configs at once.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
    pub struct SetMorseBulkRequest {
        pub start_index: u8,
        pub configs: Vec<Morse, BULK_SIZE>,
    }

    impl MaxSize for SetMorseBulkRequest {
        const POSTCARD_MAX_SIZE: usize = u8::POSTCARD_MAX_SIZE + crate::heapless_vec_max_size::<Morse, BULK_SIZE>();
    }

    /// Bulk response for getting multiple morse configs at once.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
    pub struct GetMorseBulkResponse {
        pub configs: Vec<Morse, BULK_SIZE>,
    }

    impl MaxSize for GetMorseBulkResponse {
        const POSTCARD_MAX_SIZE: usize = crate::heapless_vec_max_size::<Morse, BULK_SIZE>();
    }
}

#[cfg(feature = "bulk")]
pub use bulk::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use crate::constants::MORSE_SIZE;
    use crate::keycode::{HidKeyCode, KeyCode};
    use crate::modifier::ModifierCombination;
    use crate::morse::{MorsePattern, MorseProfile};
    use crate::protocol::rmk::test_utils::{assert_max_size_bound, round_trip};

    /// Build a `Morse` whose `actions` `LinearMap` is filled to `MORSE_SIZE`
    /// distinct entries, each using a multi-field `Action` variant so both the
    /// entry count *and* the per-entry encoded size meaningfully exercise the
    /// manual `MaxSize` impl. `MorsePattern::from_u16(0)` panics (the empty
    /// pattern is `0b1`), so patterns start at 1.
    fn full_morse() -> Morse {
        // `KeyWithModifier` carries a nested `KeyCode` enum + a `ModifierCombination`
        // bitfield, so it encodes to several bytes rather than the 1 byte of
        // `Action::No` — enough slack for `assert_max_size_bound` to catch a
        // per-element under-count.
        let action = Action::KeyWithModifier(KeyCode::Hid(HidKeyCode::A), ModifierCombination::new());
        let mut m = Morse {
            profile: MorseProfile::const_default(),
            actions: heapless::LinearMap::new(),
        };
        for i in 0..MORSE_SIZE {
            m.actions
                .insert(MorsePattern::from_u16((i + 1) as u16), action)
                .unwrap();
        }
        m
    }

    #[test]
    fn round_trip_morse() {
        round_trip(&Morse {
            profile: MorseProfile::const_default(),
            actions: heapless::LinearMap::new(),
        });
    }

    #[test]
    fn round_trip_set_morse_request() {
        let mut morse = Morse {
            profile: MorseProfile::const_default(),
            actions: heapless::LinearMap::new(),
        };
        morse.actions.insert(MorsePattern::from_u16(0b101), Action::No).unwrap();
        round_trip(&SetMorseRequest {
            index: 0,
            config: morse,
        });
    }

    #[test]
    fn round_trip_morse_max_capacity() {
        let m = full_morse();
        assert_eq!(m.actions.len(), MORSE_SIZE);
        round_trip(&m);
        assert_max_size_bound(&m);
    }

    #[cfg(feature = "bulk")]
    mod bulk {
        use heapless::Vec;

        use super::super::*;
        use super::full_morse;
        use crate::constants::BULK_SIZE;
        use crate::morse::Morse;
        use crate::protocol::rmk::test_utils::{assert_max_size_bound, round_trip};

        #[test]
        fn round_trip_set_morse_bulk_request_max_capacity() {
            let mut configs: Vec<Morse, BULK_SIZE> = Vec::new();
            for _ in 0..BULK_SIZE {
                configs.push(full_morse()).unwrap();
            }
            let req = SetMorseBulkRequest {
                start_index: u8::MAX,
                configs,
            };
            round_trip(&req);
            assert_max_size_bound(&req);
        }

        #[test]
        fn round_trip_get_morse_bulk_response_max_capacity() {
            let mut configs: Vec<Morse, BULK_SIZE> = Vec::new();
            for _ in 0..BULK_SIZE {
                configs.push(full_morse()).unwrap();
            }
            let resp = GetMorseBulkResponse { configs };
            round_trip(&resp);
            assert_max_size_bound(&resp);
        }
    }
}
