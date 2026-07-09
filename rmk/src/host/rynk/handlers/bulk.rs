//! Shared arithmetic for the bulk transfer handlers, over flat resource indices.
//! Per-resource addressing (combo/morse slot index, keymap `layer/row/col`) is
//! translated to a flat index by each handler before calling these.

use rmk_types::protocol::rynk::RynkError;

/// Clamp a bulk read to one page: at most `cap` items from flat index `start`,
/// never past `total`. An out-of-range `start` yields an empty range — the empty
/// final page that tells the host it has read everything.
pub(super) fn bulk_page(start: usize, cap: usize, total: usize) -> core::ops::Range<usize> {
    start..(start + cap).min(total)
}

/// Validate an all-or-nothing bulk write of `len` items at flat index `start`
/// into a resource of `total` items; returns the flat start offset.
pub(super) fn bulk_write_start(start: usize, len: usize, total: usize) -> Result<usize, RynkError> {
    if len == 0 || start + len > total {
        return Err(RynkError::Invalid);
    }
    Ok(start)
}
