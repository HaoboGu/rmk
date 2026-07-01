//! DFU status types.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// DFU status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DfuStatus {
    /// DFU idle / no download active
    Idle,
    /// DFU download started
    Started,
    /// Data block received and written
    Downloading,
    /// DFU download finished successfully
    Finished,
    /// A DFU error occurred
    Error,
    /// Unlock window open, waiting for unlock keys
    LockWaiting,
    /// Unlock successful, waiting for DFU download to start
    LockUnlocked,
}
