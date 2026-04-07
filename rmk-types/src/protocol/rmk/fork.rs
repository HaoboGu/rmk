//! Fork endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::fork::Fork;

/// Request payload for `SetFork`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetForkRequest {
    pub index: u8,
    pub config: Fork,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::KeyAction;
    use crate::fork::StateBits;
    use crate::modifier::ModifierCombination;
    use crate::protocol::rmk::test_utils::round_trip;

    #[test]
    fn round_trip_set_fork_request() {
        round_trip(&SetForkRequest {
            index: 2,
            config: Fork::new(
                KeyAction::No,
                KeyAction::No,
                KeyAction::No,
                StateBits::default(),
                StateBits::default(),
                ModifierCombination::new(),
                true,
            ),
        });
    }
}
