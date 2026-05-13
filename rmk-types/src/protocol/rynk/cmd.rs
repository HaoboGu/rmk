//! `Cmd` — canonical command tag space for the Rynk protocol.
//!
//! `0x0000..=0x7FFF` request/response pairs.
//! `0x8000..=0xFFFF` topics (server → host push).
//!
//! Hex groups map 1:1 to handler modules:
//!
//! | Group        | Hex     |
//! |--------------|---------|
//! | System       | `0x00xx`|
//! | Keymap       | `0x01xx`| (includes encoder)
//! | Macro        | `0x02xx`|
//! | Combo        | `0x03xx`|
//! | Morse        | `0x04xx`|
//! | Fork         | `0x05xx`|
//! | Behavior     | `0x06xx`|
//! | Connection   | `0x07xx`|
//! | Status       | `0x08xx`|
//! | Topics       | `0x80xx`|
//!
//! Lock variants (`GetLockStatus`, `UnlockRequest`, `LockRequest`) are
//! reserved for v2 at `0x0006`, `0x0007`, `0x0008`.

use strum::FromRepr;

/// Command tag carried in the header CMD field.
///
/// The wire encoding is a plain `u16 LE` written by [`FrameOps::set_cmd`](super::FrameOps::set_cmd) —
/// `Cmd` is never postcard-encoded, so no `Serialize`/`Deserialize`/`MaxSize`
/// derives are needed here.
#[repr(u16)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, FromRepr)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Cmd {
    // ── System (0x00xx) ──
    GetVersion = 0x0001,
    GetCapabilities = 0x0002,
    Reboot = 0x0003,
    BootloaderJump = 0x0004,
    StorageReset = 0x0005,
    // 0x0006..=0x0008 reserved for v2 lock gate

    // ── Keymap (0x01xx) — includes encoder ──
    GetKeyAction = 0x0101,
    SetKeyAction = 0x0102,
    GetDefaultLayer = 0x0103,
    SetDefaultLayer = 0x0104,
    GetEncoderAction = 0x0105,
    SetEncoderAction = 0x0106,
    #[cfg(feature = "bulk")]
    GetKeymapBulk = 0x0107,
    #[cfg(feature = "bulk")]
    SetKeymapBulk = 0x0108,

    // ── Macro (0x02xx) ──
    GetMacro = 0x0201,
    SetMacro = 0x0202,

    // ── Combo (0x03xx) ──
    GetCombo = 0x0301,
    SetCombo = 0x0302,
    #[cfg(feature = "bulk")]
    GetComboBulk = 0x0303,
    #[cfg(feature = "bulk")]
    SetComboBulk = 0x0304,

    // ── Morse (0x04xx) ──
    GetMorse = 0x0401,
    SetMorse = 0x0402,
    #[cfg(feature = "bulk")]
    GetMorseBulk = 0x0403,
    #[cfg(feature = "bulk")]
    SetMorseBulk = 0x0404,

    // ── Fork (0x05xx) ──
    GetFork = 0x0501,
    SetFork = 0x0502,

    // ── Behavior (0x06xx) ──
    GetBehaviorConfig = 0x0601,
    SetBehaviorConfig = 0x0602,

    // ── Connection (0x07xx) ──
    GetConnectionType = 0x0701,
    SetConnectionType = 0x0702,
    #[cfg(feature = "_ble")]
    GetBleStatus = 0x0703,
    #[cfg(feature = "_ble")]
    SwitchBleProfile = 0x0704,
    #[cfg(feature = "_ble")]
    ClearBleProfile = 0x0705,

    // ── Status (0x08xx) ──
    GetCurrentLayer = 0x0801,
    GetMatrixState = 0x0802,
    #[cfg(feature = "_ble")]
    GetBatteryStatus = 0x0803,
    #[cfg(all(feature = "_ble", feature = "split"))]
    GetPeripheralStatus = 0x0804,
    /// Latest WPM, sourced from the `WpmUpdate` topic snapshot.
    GetWpm = 0x0805,
    /// Latest sleep flag, sourced from the `SleepState` topic snapshot.
    GetSleepState = 0x0806,
    /// Latest HID LED bitmap, sourced from the `LedIndicator` topic snapshot.
    GetLedIndicator = 0x0807,

    // ── Topics (0x80xx, server → host push) ──
    LayerChange = 0x8001,
    WpmUpdate = 0x8002,
    ConnectionChange = 0x8003,
    SleepState = 0x8004,
    LedIndicator = 0x8005,
    #[cfg(feature = "_ble")]
    BatteryStatusTopic = 0x8006,
    #[cfg(feature = "_ble")]
    BleStatusChangeTopic = 0x8007,
}

impl Cmd {
    /// Returns `true` for topic / unsolicited push CMDs.
    pub const fn is_topic(self) -> bool {
        (self as u16) & 0x8000 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_mask() {
        assert!(Cmd::LayerChange.is_topic());
        assert!(Cmd::WpmUpdate.is_topic());
        assert!(!Cmd::GetVersion.is_topic());
        assert!(!Cmd::SetKeyAction.is_topic());
    }

    #[test]
    fn from_repr_unknown_returns_none() {
        assert!(Cmd::from_repr(0x0000).is_none());
        assert!(Cmd::from_repr(0x00FF).is_none());
        assert!(Cmd::from_repr(0xFFFF).is_none());
    }

    #[test]
    fn from_repr_known_round_trips() {
        // Sanity for a handful of variants — the derive guarantees every
        // compiled variant round-trips, so an exhaustive list is unnecessary.
        for cmd in [Cmd::GetVersion, Cmd::SetKeyAction, Cmd::LayerChange] {
            assert_eq!(Cmd::from_repr(cmd as u16), Some(cmd));
        }
    }
}
