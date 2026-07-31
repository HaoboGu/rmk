use rmk_types::protocol::rynk::RynkError;
use serde::de::DeserializeOwned;

/// Decode the next postcard value, advancing the cursor.
pub(super) fn take_element<T: DeserializeOwned>(cursor: &mut &[u8]) -> Result<T, RynkError> {
    let (value, rest) = postcard::take_from_bytes::<T>(cursor).map_err(|_| RynkError::Malformed)?;
    *cursor = rest;
    Ok(value)
}

/// Clamp a bulk read to at most `cap` items from flat index `start`, never
/// past `total`. Zero `cap` with items remaining is `Busy` — `cap` reflects
/// transient reply-window space, so the host retries.
pub(super) fn bulk_page(start: usize, cap: usize, total: usize) -> Result<core::ops::Range<usize>, RynkError> {
    if cap == 0 && start < total {
        return Err(RynkError::Busy);
    }
    Ok(start..(start + cap).min(total))
}

/// Decode a bulk write's element `Vec` (postcard: varint count + elements)
/// as `(flat_index, element)` pairs, validating the whole payload up front
/// so malformed input cannot cause a partial write.
pub(super) fn take_bulk<'a, T: DeserializeOwned + 'a>(
    cursor: &mut &'a [u8],
    start: usize,
    total: usize,
) -> Result<impl Iterator<Item = (usize, T)> + 'a, RynkError> {
    let count = take_element::<u16>(cursor)? as usize;
    if count == 0 || start + count > total {
        return Err(RynkError::Invalid);
    }
    let mut elements = *cursor;
    for _ in 0..count {
        take_element::<T>(cursor)?;
    }
    Ok((start..start + count).zip(core::iter::from_fn(move || take_element::<T>(&mut elements).ok())))
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec::Vec;

    use rmk_types::action::KeyAction;
    use rmk_types::protocol::rynk::MAX_BULK_KEYS;

    use super::*;

    #[test]
    fn bulk_write_decodes_every_element_before_mutation() {
        let count = MAX_BULK_KEYS + 1;
        let mut bytes = [0; 512];
        let mut len = postcard::to_slice(&(count as u16), &mut bytes).unwrap().len();
        for _ in 0..count {
            len += postcard::to_slice(&KeyAction::No, &mut bytes[len..]).unwrap().len();
        }
        let mut input = &bytes[..len];
        let items: Vec<_> = take_bulk::<KeyAction>(&mut input, 0, count).unwrap().collect();
        assert_eq!(items.len(), count);

        let mut len = postcard::to_slice(&2u16, &mut bytes).unwrap().len();
        len += postcard::to_slice(&KeyAction::No, &mut bytes[len..]).unwrap().len();
        bytes[len] = 0x7f;
        let mut input = &bytes[..=len];
        assert!(matches!(
            take_bulk::<KeyAction>(&mut input, 0, 2),
            Err(RynkError::Malformed)
        ));
    }
}
