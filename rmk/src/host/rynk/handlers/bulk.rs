use rmk_types::protocol::rynk::RynkError;
use serde::de::DeserializeOwned;

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

/// Valid bulk counts fit in `u16`.
pub(super) fn take_seq_len(bytes: &[u8]) -> Result<(usize, &[u8]), RynkError> {
    let (len, rest) = postcard::take_from_bytes::<u16>(bytes).map_err(|_| RynkError::Malformed)?;
    Ok((len as usize, rest))
}

pub(super) fn take_element<T: DeserializeOwned>(cursor: &mut &[u8]) -> Result<T, RynkError> {
    let (value, rest) = postcard::take_from_bytes::<T>(cursor).map_err(|_| RynkError::Malformed)?;
    *cursor = rest;
    Ok(value)
}

/// Validates all elements first so malformed input cannot cause a partial write.
pub(super) fn validate_bulk_elements<T: DeserializeOwned>(mut bytes: &[u8], count: usize) -> Result<(), RynkError> {
    for _ in 0..count {
        take_element::<T>(&mut bytes)?;
    }
    Ok(())
}
