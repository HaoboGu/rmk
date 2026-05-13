//! Compile-time wire buffer sizing.
//!
//! `RYNK_MIN_BUFFER_SIZE` is the minimum buffer size — header + maximum
//! payload across every wire request and response. Firmware uses this
//! as the floor for its RX/TX buffers; user can configure
//! `[rmk] rynk_buffer_size` in `keyboard.toml` to enlarge it, but never
//! shrink below this floor (enforced by `const _: () = assert!(...)`
//! on the firmware side).
//!
//! Every wire type derives `postcard::experimental::max_size::MaxSize`,
//! so the entire computation resolves at compile time.
//!
//! ## Response wrapping
//!
//! Every response payload is `Result<T, RynkError>`, so the response sizing
//! folds over `Result<T, RynkError>::POSTCARD_MAX_SIZE` rather than
//! `T::POSTCARD_MAX_SIZE` directly. Postcard's enum tag is 1 byte, so
//! each response is at most `1 + T::POSTCARD_MAX_SIZE` bytes.

use postcard::experimental::max_size::MaxSize;

use super::*;

/// `const fn` max used to fold over all wire types at compile time.
const fn max_const(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

/// Maximum postcard-encoded payload size across every Rynk wire type.
///
/// Folds over all responses (wrapped in `Result<T, RynkError>`) and all
/// requests where the request can be larger than any response (e.g.
/// bulk-set requests).
pub const RYNK_MAX_PAYLOAD: usize = {
    let mut m = 0usize;

    // ── responses (all wrapped in Result<T, RynkError>) ──
    m = max_const(m, <Result<ProtocolVersion, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<DeviceCapabilities, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<LockStatus, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<UnlockChallenge, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<BehaviorConfig, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<crate::action::KeyAction, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<crate::action::EncoderAction, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<crate::combo::Combo, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<crate::morse::Morse, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<crate::fork::Fork, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<crate::connection::ConnectionType, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<MacroData, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <Result<MatrixState, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    // Canonical Set* reply.
    m = max_const(m, <Result<(), RynkError> as MaxSize>::POSTCARD_MAX_SIZE);

    #[cfg(feature = "_ble")]
    {
        m = max_const(m, <Result<crate::battery::BatteryStatus, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
        m = max_const(m, <Result<crate::ble::BleStatus, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    }
    #[cfg(all(feature = "_ble", feature = "split"))]
    {
        m = max_const(m, <Result<PeripheralStatus, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    }

    // ── requests that can be larger than any response ──
    m = max_const(m, <SetKeyRequest as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <SetEncoderRequest as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <GetMacroRequest as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <SetMacroRequest as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <SetComboRequest as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <SetMorseRequest as MaxSize>::POSTCARD_MAX_SIZE);
    m = max_const(m, <SetForkRequest as MaxSize>::POSTCARD_MAX_SIZE);

    #[cfg(feature = "bulk")]
    {
        m = max_const(m, <GetKeymapBulkRequest as MaxSize>::POSTCARD_MAX_SIZE);
        m = max_const(m, <Result<GetKeymapBulkResponse, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
        m = max_const(m, <SetKeymapBulkRequest as MaxSize>::POSTCARD_MAX_SIZE);
        m = max_const(m, <SetComboBulkRequest as MaxSize>::POSTCARD_MAX_SIZE);
        m = max_const(m, <Result<GetComboBulkResponse, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
        m = max_const(m, <SetMorseBulkRequest as MaxSize>::POSTCARD_MAX_SIZE);
        m = max_const(m, <Result<GetMorseBulkResponse, RynkError> as MaxSize>::POSTCARD_MAX_SIZE);
    }

    m
};

/// Minimum buffer size required to hold any single Rynk frame
/// (header + max-payload). User-configured `RYNK_BUFFER_SIZE` must
/// not be smaller than this.
pub const RYNK_MIN_BUFFER_SIZE: usize = RYNK_HEADER_SIZE + RYNK_MAX_PAYLOAD;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rynk_min_buffer_size_is_at_least_one_header() {
        assert!(RYNK_MIN_BUFFER_SIZE >= RYNK_HEADER_SIZE);
    }

    #[test]
    fn rynk_min_buffer_size_covers_largest_known_response() {
        // `DeviceCapabilities` is one of the largest single-message responses
        // when bulk is disabled; the min buffer must hold its wrapped form
        // plus header.
        let wrapped = <Result<DeviceCapabilities, RynkError> as MaxSize>::POSTCARD_MAX_SIZE;
        assert!(RYNK_MAX_PAYLOAD >= wrapped);
        assert!(RYNK_MIN_BUFFER_SIZE >= wrapped + RYNK_HEADER_SIZE);
    }

    #[test]
    fn response_wrapping_adds_one_byte() {
        // Postcard's Result tag is 1 byte. The wrapped size of any
        // non-trivial T must equal `1 + T::POSTCARD_MAX_SIZE`.
        let bare = <DeviceCapabilities as MaxSize>::POSTCARD_MAX_SIZE;
        let wrapped = <Result<DeviceCapabilities, RynkError> as MaxSize>::POSTCARD_MAX_SIZE;
        assert_eq!(wrapped, bare + 1);
    }
}
