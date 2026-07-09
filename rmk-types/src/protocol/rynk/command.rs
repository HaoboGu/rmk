//! The Rynk command identifier and the command table.
//!
//! [`Cmd`] is the 16-bit identifier carried in the header CMD field. The most
//! significant bit (`0x8000`) acts as a flag to identify "Topics":
//!
//! - `0x0000..=0x7FFF` (Bit 15 = 0): Request/Response pairs.
//! - `0x8000..=0xFFFF` (Bit 15 = 1): Topics (Server -> Host push).
//!

use super::endpoint::{Endpoint, Topic, max_const};
use super::message::RynkMessage;
use super::{
    BehaviorConfig, DeviceCapabilities, DeviceInfo, GetEncoderRequest, GetMacroRequest, KeyPosition, LayoutChunk,
    LockStatus, MacroData, MatrixState, ProtocolVersion, RynkError, SetComboRequest, SetEncoderRequest, SetForkRequest,
    SetKeyRequest, SetMacroRequest, SetMorseRequest, StorageResetMode,
};
#[cfg(feature = "bulk")]
use super::{
    GetComboBulkRequest, GetComboBulkResponse, GetKeymapBulkRequest, GetKeymapBulkResponse, GetMorseBulkRequest,
    GetMorseBulkResponse, SetComboBulkRequest, SetKeymapBulkRequest, SetMorseBulkRequest,
};
use crate::action::{EncoderAction, KeyAction};
#[cfg(feature = "_ble")]
use crate::battery::BatteryStatus;
#[cfg(feature = "_ble")]
use crate::ble::BleStatus;
use crate::combo::Combo;
use crate::connection::{ConnectionStatus, ConnectionType};
use crate::fork::Fork;
use crate::led_indicator::LedIndicator;
use crate::morse::Morse;
#[cfg(all(feature = "_ble", feature = "split"))]
use crate::protocol::rynk::PeripheralStatus;

/// CMD high bit marking a topic (server → host push).
const RYNK_TOPIC_BIT: u16 = 0x8000;

/// The command identifier carried in the header CMD field. The named
/// `Cmd` constants are generated from the `endpoints!`/`topics!` table below.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Cmd(u16);

impl Cmd {
    /// Build a `Cmd` from its raw wire value.
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Build a `Cmd` from the header's little-endian CMD bytes.
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

/// Compile-time guard: Check whether the command value is unique.
const fn assert_unique(cmds: &[u16]) {
    let mut i = 0;
    while i < cmds.len() {
        let mut j = i + 1;
        while j < cmds.len() {
            core::assert!(cmds[i] != cmds[j], "duplicate CMD value in the command table");
            j += 1;
        }
        i += 1;
    }
}

/// Macro for defining the endpoint (request/response) table.
///
/// A row may carry an optional `@bulk` marker before its name. Such an
/// endpoint is generated like any other, but is left out of the
/// `MAX_ENDPOINT_PAYLOAD` fold: its payload is sized dynamically (bulk transfer,
/// whose element count tracks `RYNK_BUFFER_SIZE`), so it must not drive the
/// buffer floor — the buffer drives it, not the other way around.
macro_rules! endpoints {
    // Fold contribution of one row: its max payload, or 0 when marked `@bulk`.
    (@floor $name:ident) => { <$name as Endpoint>::MAX_PAYLOAD };
    (@floor $marker:ident $name:ident) => { 0 };
    ($( $(#[$meta:meta])* $(@ $marker:ident)? $name:ident = $cmd:literal : $req:ty => $resp:ty; )*) => {
        #[allow(non_upper_case_globals)]
        impl Cmd {
            $( $(#[$meta])* pub const $name: Self = Cmd::from_raw($cmd); )*
        }
        $(
            $(#[$meta])*
            pub enum $name {}
            $(#[$meta])*
            impl Endpoint for $name {
                const CMD: Cmd = Cmd::$name;
                type Request = $req;
                type Response = $resp;
            }
        )*
        const _: () = {
            $( core::assert!(!Cmd::from_raw($cmd).is_topic(), "request CMD value in the topic range"); )*
            assert_unique(&[$($cmd),*]);
        };
        /// Largest non-bulk payload across the endpoint table — the buffer floor.
        #[allow(unused_doc_comments)] // row docs also land on the fold statements
        const MAX_ENDPOINT_PAYLOAD: usize = {
            let mut m = 0;
            $( $(#[$meta])* { m = max_const(m, endpoints!(@floor $($marker)? $name)); } )*
            m
        };
    };
}

/// Macro for defining the topic table.
macro_rules! topics {
    ($( $(#[$meta:meta])* $name:ident = $cmd:literal : $payload:ty; )*) => {
        #[allow(non_upper_case_globals)]
        impl Cmd {
            $( $(#[$meta])* pub const $name: Self = Cmd::from_raw($cmd); )*
        }
        $(
            $(#[$meta])*
            pub enum $name {}
            $(#[$meta])*
            impl Topic for $name {
                const CMD: Cmd = Cmd::$name;
                type Payload = $payload;
            }
        )*
        const _: () = {
            $( core::assert!(Cmd::from_raw($cmd).is_topic(), "topic CMD value outside the topic range"); )*
            assert_unique(&[$($cmd),*]);
        };
        /// Largest payload across the whole topic table.
        #[allow(unused_doc_comments)]
        const MAX_TOPIC_PAYLOAD: usize = {
            let mut m = 0;
            $( $(#[$meta])* { m = max_const(m, <$name as Topic>::MAX_PAYLOAD); } )*
            m
        };

        /// A decoded topic push (server → host), one variant per row of the
        /// topic table above — generated from it. `Serialize` lets the host
        /// re-emit a decoded topic as JSON (every payload is already a wire type).
        #[derive(Debug, Clone, serde::Serialize)]
        #[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
        #[cfg_attr(feature = "wasm", tsify(into_wasm_abi))]
        pub enum TopicEvent {
            $( $(#[$meta])* $name($payload), )*
        }

        impl TopicEvent {
            /// The `Cmd` this event is pushed under.
            pub fn cmd(&self) -> Cmd {
                match self {
                    $( $(#[$meta])* TopicEvent::$name(_) => Cmd::$name, )*
                }
            }

            /// Decode a topic frame's `payload` as the topic named by `cmd`.
            /// `None` for a `cmd` outside the topic table, or a payload that
            /// fails to decode. Trailing bytes are ignored.
            pub fn decode(cmd: Cmd, payload: &[u8]) -> Option<Self> {
                match cmd {
                    $( $(#[$meta])* Cmd::$name => postcard::take_from_bytes::<$payload>(payload)
                        .ok()
                        .map(|(v, _)| TopicEvent::$name(v)), )*
                    _ => None,
                }
            }

            /// Encode this event into `buf` as a topic frame.
            /// Returns the message view; the caller sends `&buf[..msg.frame_len()]`.
            pub fn encode<'a>(&self, buf: &'a mut [u8]) -> Result<RynkMessage<'a>, RynkError> {
                match self {
                    $( $(#[$meta])* TopicEvent::$name(v) => RynkMessage::build_topic::<$name>(buf, v), )*
                }
            }
        }
    };
}

// Define endpoints: `Name = value: Request => Response;`
endpoints! {
    // ── System (0x00xx); 0x0009 reserved for layout ──
    GetVersion = 0x0001: () => ProtocolVersion;
    GetCapabilities = 0x0002: () => DeviceCapabilities;
    Reboot = 0x0003: () => ();
    BootloaderJump = 0x0004: () => ();
    StorageReset = 0x0005: StorageResetMode => ();
    // Lock gate. All three stay dispatchable while locked.
    /// Pure read of the current lock state — no side effects.
    GetLockStatus = 0x0006: () => LockStatus;
    /// Arms/refreshes the unlock attempt and samples the held challenge keys.
    UnlockPoll = 0x0007: () => LockStatus;
    /// Relock immediately.
    Lock = 0x0008: () => ();
    /// Get layout blob chunk. `u32` is the byte offset.
    GetLayout = 0x0009: u32 => LayoutChunk;
    /// Identity strings and USB ids; feature gating stays in `GetCapabilities`.
    GetDeviceInfo = 0x000A: () => DeviceInfo;

    // ── Keymap (0x01xx) — includes encoder ──
    GetKeyAction = 0x0101: KeyPosition => KeyAction;
    SetKeyAction = 0x0102: SetKeyRequest => ();
    GetDefaultLayer = 0x0103: () => u8;
    SetDefaultLayer = 0x0104: u8 => ();
    GetEncoderAction = 0x0105: GetEncoderRequest => EncoderAction;
    SetEncoderAction = 0x0106: SetEncoderRequest => ();
    #[cfg(feature = "bulk")]
    @bulk GetKeymapBulk = 0x0107: GetKeymapBulkRequest => GetKeymapBulkResponse;
    #[cfg(feature = "bulk")]
    @bulk SetKeymapBulk = 0x0108: SetKeymapBulkRequest => ();

    // ── Macro (0x02xx) ──
    GetMacro = 0x0201: GetMacroRequest => MacroData;
    SetMacro = 0x0202: SetMacroRequest => ();

    // ── Combo (0x03xx) ──
    GetCombo = 0x0301: u8 => Combo;
    SetCombo = 0x0302: SetComboRequest => ();
    #[cfg(feature = "bulk")]
    @bulk GetComboBulk = 0x0303: GetComboBulkRequest => GetComboBulkResponse;
    #[cfg(feature = "bulk")]
    @bulk SetComboBulk = 0x0304: SetComboBulkRequest => ();

    // ── Morse (0x04xx) ──
    GetMorse = 0x0401: u8 => Morse;
    SetMorse = 0x0402: SetMorseRequest => ();
    #[cfg(feature = "bulk")]
    @bulk GetMorseBulk = 0x0403: GetMorseBulkRequest => GetMorseBulkResponse;
    #[cfg(feature = "bulk")]
    @bulk SetMorseBulk = 0x0404: SetMorseBulkRequest => ();

    // ── Fork (0x05xx) ──
    GetFork = 0x0501: u8 => Fork;
    SetFork = 0x0502: SetForkRequest => ();

    // ── Behavior (0x06xx) ──
    GetBehaviorConfig = 0x0601: () => BehaviorConfig;
    SetBehaviorConfig = 0x0602: BehaviorConfig => ();

    // ── Connection (0x07xx) ──
    GetConnectionType = 0x0701: () => ConnectionType;
    /// Full `ConnectionStatus` snapshot.
    GetConnectionStatus = 0x0702: () => ConnectionStatus;
    #[cfg(feature = "_ble")]
    GetBleStatus = 0x0703: () => BleStatus;
    #[cfg(feature = "_ble")]
    SwitchBleProfile = 0x0704: u8 => ();
    #[cfg(feature = "_ble")]
    ClearBleProfile = 0x0705: u8 => ();

    // ── Status (0x08xx) ──
    GetCurrentLayer = 0x0801: () => u8;
    GetMatrixState = 0x0802: () => MatrixState;
    #[cfg(feature = "_ble")]
    GetBatteryStatus = 0x0803: () => BatteryStatus;
    #[cfg(all(feature = "_ble", feature = "split"))]
    GetPeripheralStatus = 0x0804: u8 => PeripheralStatus;
    /// Latest WPM, sourced from the `WpmUpdate` topic snapshot.
    GetWpm = 0x0805: () => u16;
    /// Latest sleep flag, sourced from the `SleepState` topic snapshot.
    GetSleepState = 0x0806: () => bool;
    /// Latest HID LED bitmap, sourced from the `LedIndicatorChange` topic snapshot.
    GetLedIndicator = 0x0807: () => LedIndicator;
}

// Define topics: `Name = value: Payload;`
topics! {
    // ── Topics (0x80xx, server → host push) ──
    LayerChange = 0x8001: u8;
    WpmUpdate = 0x8002: u16;
    ConnectionChange = 0x8003: ConnectionStatus;
    SleepState = 0x8004: bool;
    LedIndicatorChange = 0x8005: LedIndicator;
    #[cfg(feature = "_ble")]
    BatteryStatusChange = 0x8006: BatteryStatus;
}

/// Maximum postcard-encoded payload size across every non-`@bulk` Rynk
/// message, folded from the tables above so adding a command can never
/// under-size the buffer. Bulk endpoints are excluded (see [`endpoints`]);
/// their size derives from the buffer instead.
pub const RYNK_MAX_PAYLOAD: usize = max_const(MAX_ENDPOINT_PAYLOAD, MAX_TOPIC_PAYLOAD);

// Bulk transfer element counts are derived from the Rynk buffer size, not
// configured: a bulk message exists to fill the buffer, so its count is whatever
// fits. `rmk-types/build.rs` emits `BULK_SIZE`/`BULK_KEYMAP_SIZE` on the firmware
// as calls to these `const fn`s over `RYNK_BUFFER_SIZE`, then reports them to the
// host via `GetCapabilities`. They live here, not in `build.rs`, because they
// need the payload types' `POSTCARD_MAX_SIZE`, which `build.rs` cannot see.
#[cfg(feature = "bulk")]
mod bulk_capacity {
    use postcard::experimental::max_size::MaxSize;

    use super::super::endpoint::{max_const, min_const};
    use super::super::message::RYNK_HEADER_SIZE;
    use crate::action::KeyAction;
    use crate::combo::Combo;
    use crate::morse::Morse;

    /// Reported bulk counts are `u8`, so a single message never carries more
    /// than this many elements regardless of how large the buffer is.
    const BULK_COUNT_CEILING: usize = u8::MAX as usize;

    /// Default Rynk RX/TX buffer size (bytes) when `rynk_buffer_size` is unset.
    /// Because the bulk counts scale with the buffer, this also sets default bulk
    /// throughput: 512 B holds several combos/morses and a long keymap run per
    /// message while staying modest on RAM.
    pub const RYNK_DEFAULT_BUFFER_SIZE: usize = 512;

    /// Elements of `item_size` bytes that fit in one `buffer`-byte frame after
    /// reserving the header, `fixed` non-element bytes, and the widest length
    /// prefix a `u8` count can take (the varint of 255 is 2 bytes). Capped at
    /// `u8::MAX`; may be 0 for a buffer too small to hold one element, which the
    /// `BULK_SIZE >= 1` build assert then rejects with a clear message.
    const fn items_that_fit(buffer: usize, item_size: usize, fixed: usize) -> usize {
        let overhead = RYNK_HEADER_SIZE + fixed + crate::varint_max_size(BULK_COUNT_CEILING);
        min_const(buffer.saturating_sub(overhead) / item_size, BULK_COUNT_CEILING)
    }

    /// Combos/morses per bulk frame. Sized by the larger of `Combo`/`Morse` so
    /// both bulk endpoints fit; the one fixed byte is `start_index` on the
    /// request / the `Result` tag on the response.
    pub const fn bulk_size_for_buffer(buffer: usize) -> usize {
        let item = max_const(Combo::POSTCARD_MAX_SIZE, Morse::POSTCARD_MAX_SIZE);
        items_that_fit(buffer, item, 1)
    }

    /// Keymap keys per bulk frame. Keys (`KeyAction`) are far smaller than a
    /// `Combo`, so a keymap run naturally outnumbers a combo/morse run in the
    /// same buffer; the three fixed bytes are `layer`/`start_row`/`start_col`.
    pub const fn bulk_keymap_size_for_buffer(buffer: usize) -> usize {
        items_that_fit(buffer, KeyAction::POSTCARD_MAX_SIZE, 3)
    }
}

#[cfg(feature = "bulk")]
pub use bulk_capacity::{RYNK_DEFAULT_BUFFER_SIZE, bulk_keymap_size_for_buffer, bulk_size_for_buffer};

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::format;

    use postcard::experimental::max_size::MaxSize;

    use super::*;
    use crate::protocol::rynk::{RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkHeader};

    #[test]
    fn topic_mask_is_the_high_bit() {
        assert!(Cmd::from_raw(0x8000).is_topic());
        assert!(Cmd::from_raw(0x80ff).is_topic());
        assert!(!Cmd::from_raw(0x0001).is_topic());
        assert!(!Cmd::from_raw(0x7fff).is_topic());
    }

    #[test]
    fn raw_values_round_trip() {
        for cmd in [Cmd::from_raw(0x0001), Cmd::from_raw(0x8001), Cmd::from_raw(0xffff)] {
            assert_eq!(Cmd::from_raw(cmd.raw()), cmd);
            assert_eq!(Cmd::from_le_bytes(cmd.to_le_bytes()), cmd);
        }
    }

    #[test]
    fn debug_is_compact_raw_value() {
        assert_eq!(format!("{:?}", Cmd::from_raw(0x0001)), "Cmd(0x0001)");
        assert_eq!(format!("{:?}", Cmd::from_raw(0x80ff)), "Cmd(0x80ff)");
    }

    #[test]
    fn table_cmds_land_in_their_ranges() {
        assert!(Cmd::LayerChange.is_topic());
        assert!(Cmd::WpmUpdate.is_topic());
        assert!(!Cmd::GetVersion.is_topic());
        assert!(!Cmd::SetKeyAction.is_topic());
    }

    #[test]
    fn topic_event_round_trips_through_the_wire() {
        // The generated enum encodes to a topic frame the host decodes back to
        // the same variant — the producer and consumer halves share one table.
        let mut buf = [0u8; RYNK_MIN_BUFFER_SIZE];
        let ev = TopicEvent::LayerChange(7);
        let frame_len = ev.encode(&mut buf).unwrap().frame_len();

        let header = RynkHeader::parse(buf.first_chunk::<RYNK_HEADER_SIZE>().unwrap());
        assert_eq!(header.cmd, Cmd::LayerChange);
        assert_eq!(header.cmd, ev.cmd());
        assert_eq!(header.seq, 0, "topics push with SEQ 0");

        let decoded = TopicEvent::decode(header.cmd, &buf[RYNK_HEADER_SIZE..frame_len]);
        assert!(matches!(decoded, Some(TopicEvent::LayerChange(7))));
    }

    #[test]
    fn topic_event_decode_rejects_non_topic_and_garbage() {
        // A request-range cmd is not in the topic table.
        assert!(TopicEvent::decode(Cmd::GetVersion, &[]).is_none());
        // A known topic cmd whose payload can't decode (LayerChange needs a byte).
        assert!(TopicEvent::decode(Cmd::LayerChange, &[]).is_none());
    }

    #[test]
    fn response_wrapping_adds_one_byte() {
        // Postcard's Result tag is 1 byte. The wrapped size of any
        // non-trivial T must equal `1 + T::POSTCARD_MAX_SIZE`.
        let bare = <DeviceCapabilities as MaxSize>::POSTCARD_MAX_SIZE;
        let wrapped = <Result<DeviceCapabilities, RynkError> as MaxSize>::POSTCARD_MAX_SIZE;
        assert_eq!(wrapped, bare + 1);
    }

    /// The buffer-derived bulk counts stay within `[1, u8::MAX]`, grow with the
    /// buffer, and — crucially — their worst-case encoded frame fits the buffer
    /// they were derived from. That fit is what lets the firmware serve a full
    /// bulk message out of its `RYNK_BUFFER_SIZE` buffer.
    #[cfg(feature = "bulk")]
    #[test]
    fn bulk_counts_derive_from_buffer_and_fit() {
        use super::{bulk_keymap_size_for_buffer, bulk_size_for_buffer};
        use crate::action::KeyAction;
        use crate::combo::Combo;
        use crate::morse::Morse;
        use crate::varint_max_size;

        const U8_MAX: usize = u8::MAX as usize;
        // Clamp to the u8 report width for a large buffer; 0 for a buffer too small
        // to hold even one element (the `BULK_SIZE >= 1` build assert rejects that).
        assert_eq!(bulk_size_for_buffer(usize::MAX / 2), U8_MAX);
        assert_eq!(bulk_keymap_size_for_buffer(usize::MAX / 2), U8_MAX);
        assert_eq!(bulk_size_for_buffer(0), 0);
        assert_eq!(bulk_keymap_size_for_buffer(0), 0);

        let combo_item = max_const(Combo::POSTCARD_MAX_SIZE, Morse::POSTCARD_MAX_SIZE);
        let (mut prev_c, mut prev_k) = (0, 0);
        for buffer in (0..=200_000).step_by(8) {
            let c = bulk_size_for_buffer(buffer);
            let k = bulk_keymap_size_for_buffer(buffer);
            assert!(c >= prev_c && k >= prev_k, "counts must not shrink as the buffer grows");
            (prev_c, prev_k) = (c, k);

            // Every non-zero count's worst-case frame — header + the fixed
            // request/response bytes + the widest u8 length prefix + elements —
            // must fit the buffer it was derived from.
            if c >= 1 {
                let frame = RYNK_HEADER_SIZE + 1 + varint_max_size(c) + c * combo_item;
                assert!(frame <= buffer, "combo/morse bulk frame {frame} > buffer {buffer}");
            }
            if k >= 1 {
                let frame = RYNK_HEADER_SIZE + 3 + varint_max_size(k) + k * KeyAction::POSTCARD_MAX_SIZE;
                assert!(frame <= buffer, "keymap bulk frame {frame} > buffer {buffer}");
            }
        }
    }
}
