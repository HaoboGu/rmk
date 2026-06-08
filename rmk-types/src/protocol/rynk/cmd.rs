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

/// CMD high bit marking a topic (server → host push). Requests/responses live
/// in `0x0000..=0x7FFF`; topics in `0x8000..=0xFFFF`.
const RYNK_TOPIC_BIT: u16 = 0x8000;

/// Command tag carried in the header CMD field.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Cmd(u16);

#[allow(non_upper_case_globals)]
impl Cmd {
    // ── System (0x00xx) ──
    pub const GetVersion: Self = Self(0x0001);
    pub const GetCapabilities: Self = Self(0x0002);
    pub const Reboot: Self = Self(0x0003);
    pub const BootloaderJump: Self = Self(0x0004);
    pub const StorageReset: Self = Self(0x0005);
    // 0x0006..=0x0008 reserved for v2 lock gate

    // ── Keymap (0x01xx) — includes encoder ──
    pub const GetKeyAction: Self = Self(0x0101);
    pub const SetKeyAction: Self = Self(0x0102);
    pub const GetDefaultLayer: Self = Self(0x0103);
    pub const SetDefaultLayer: Self = Self(0x0104);
    pub const GetEncoderAction: Self = Self(0x0105);
    pub const SetEncoderAction: Self = Self(0x0106);
    #[cfg(feature = "bulk")]
    pub const GetKeymapBulk: Self = Self(0x0107);
    #[cfg(feature = "bulk")]
    pub const SetKeymapBulk: Self = Self(0x0108);

    // ── Macro (0x02xx) ──
    pub const GetMacro: Self = Self(0x0201);
    pub const SetMacro: Self = Self(0x0202);

    // ── Combo (0x03xx) ──
    pub const GetCombo: Self = Self(0x0301);
    pub const SetCombo: Self = Self(0x0302);
    #[cfg(feature = "bulk")]
    pub const GetComboBulk: Self = Self(0x0303);
    #[cfg(feature = "bulk")]
    pub const SetComboBulk: Self = Self(0x0304);

    // ── Morse (0x04xx) ──
    pub const GetMorse: Self = Self(0x0401);
    pub const SetMorse: Self = Self(0x0402);
    #[cfg(feature = "bulk")]
    pub const GetMorseBulk: Self = Self(0x0403);
    #[cfg(feature = "bulk")]
    pub const SetMorseBulk: Self = Self(0x0404);

    // ── Fork (0x05xx) ──
    pub const GetFork: Self = Self(0x0501);
    pub const SetFork: Self = Self(0x0502);

    // ── Behavior (0x06xx) ──
    pub const GetBehaviorConfig: Self = Self(0x0601);
    pub const SetBehaviorConfig: Self = Self(0x0602);

    // ── Connection (0x07xx) ──
    pub const GetConnectionType: Self = Self(0x0701);
    /// Full `ConnectionStatus` snapshot — same payload the `ConnectionChange`
    /// topic pushes, so a host can recover a missed push.
    pub const GetConnectionStatus: Self = Self(0x0702);
    #[cfg(feature = "_ble")]
    pub const GetBleStatus: Self = Self(0x0703);
    #[cfg(feature = "_ble")]
    pub const SwitchBleProfile: Self = Self(0x0704);
    #[cfg(feature = "_ble")]
    pub const ClearBleProfile: Self = Self(0x0705);

    // ── Status (0x08xx) ──
    pub const GetCurrentLayer: Self = Self(0x0801);
    pub const GetMatrixState: Self = Self(0x0802);
    #[cfg(feature = "_ble")]
    pub const GetBatteryStatus: Self = Self(0x0803);
    #[cfg(all(feature = "_ble", feature = "split"))]
    pub const GetPeripheralStatus: Self = Self(0x0804);
    /// Latest WPM, sourced from the `WpmUpdate` topic snapshot.
    pub const GetWpm: Self = Self(0x0805);
    /// Latest sleep flag, sourced from the `SleepState` topic snapshot.
    pub const GetSleepState: Self = Self(0x0806);
    /// Latest HID LED bitmap, sourced from the `LedIndicator` topic snapshot.
    pub const GetLedIndicator: Self = Self(0x0807);

    // ── Topics (0x80xx, server → host push) ──
    pub const LayerChange: Self = Self(0x8001);
    pub const WpmUpdate: Self = Self(0x8002);
    pub const ConnectionChange: Self = Self(0x8003);
    pub const SleepState: Self = Self(0x8004);
    pub const LedIndicator: Self = Self(0x8005);
    #[cfg(feature = "_ble")]
    pub const BatteryStatusTopic: Self = Self(0x8006);

    /// Build a command tag from its raw wire value.
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Build a command tag from the header's little-endian CMD bytes.
    pub const fn from_le_bytes(bytes: [u8; 2]) -> Self {
        Self(u16::from_le_bytes(bytes))
    }

    /// Return the raw wire value.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Return the header's little-endian CMD bytes.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }

    /// Returns `true` for topic / unsolicited push CMDs (high bit set).
    pub const fn is_topic(self) -> bool {
        self.0 & RYNK_TOPIC_BIT != 0
    }
}

impl core::fmt::Debug for Cmd {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Cmd(0x{:04x})", self.0)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Cmd {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "Cmd(0x{=u16:04x})", self.0)
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::format;

    use super::*;

    #[test]
    fn topic_mask() {
        assert!(Cmd::LayerChange.is_topic());
        assert!(Cmd::WpmUpdate.is_topic());
        assert!(Cmd::from_raw(0x80ff).is_topic());
        assert!(!Cmd::GetVersion.is_topic());
        assert!(!Cmd::SetKeyAction.is_topic());
    }

    #[test]
    fn raw_values_round_trip() {
        for cmd in [
            Cmd::GetVersion,
            Cmd::SetKeyAction,
            Cmd::LayerChange,
            Cmd::from_raw(0xffff),
        ] {
            assert_eq!(Cmd::from_raw(cmd.raw()), cmd);
            assert_eq!(Cmd::from_le_bytes(cmd.to_le_bytes()), cmd);
        }
    }

    #[test]
    fn debug_is_compact_raw_value() {
        assert_eq!(format!("{:?}", Cmd::GetVersion), "Cmd(0x0001)");
        assert_eq!(format!("{:?}", Cmd::from_raw(0x80ff)), "Cmd(0x80ff)");
    }
}
