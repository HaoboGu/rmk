//! Stenography (Plover HID) key identifiers.
//!
//! Each [`StenoKey`] is an index 0..=63 into the 64-bit chord bitmap that
//! Plover HID delivers to the host. The order of the chart matches the
//! canonical Plover HID specification at
//! <https://github.com/dnaq/plover-machine-hid>: index 0 is `S1-` and is
//! the most significant bit of byte 1 of the wire report; index 63 (`X26`)
//! is the least significant bit of byte 8.
//!
//! The standard Ward-Stone-Ireland keys come first (`S1-` through `#1`,
//! indices 0-22), followed by the extended steno keys (`S2-`, `*2`-`*4`,
//! `#2`-`#C`, indices 23-37), followed by 26 vendor-defined "extra" keys
//! `X1`..`X26` (indices 38-63).

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// A single steno key, identified by its position (0..=63) in the canonical
/// Plover HID key chart. See module docs for the full list.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
pub struct StenoKey(pub u8);

impl StenoKey {
    // Standard Ward-Stone-Ireland keys (chart indices 0-22)
    pub const S1: Self = Self(0);
    pub const T: Self = Self(1);
    pub const K: Self = Self(2);
    pub const P: Self = Self(3);
    pub const W: Self = Self(4);
    pub const H: Self = Self(5);
    // Left-hand R; right-hand keys are prefixed `R` (e.g. `RR` = right R).
    pub const R: Self = Self(6);
    pub const A: Self = Self(7);
    pub const O: Self = Self(8);
    pub const STAR1: Self = Self(9);
    pub const RE: Self = Self(10);
    pub const RU: Self = Self(11);
    pub const RF: Self = Self(12);
    pub const RR: Self = Self(13);
    pub const RP: Self = Self(14);
    pub const RB: Self = Self(15);
    pub const RL: Self = Self(16);
    pub const RG: Self = Self(17);
    pub const RT: Self = Self(18);
    pub const RS: Self = Self(19);
    pub const RD: Self = Self(20);
    pub const RZ: Self = Self(21);
    pub const NUM1: Self = Self(22);

    // Extended steno keys (chart indices 23-37)
    pub const S2: Self = Self(23);
    pub const STAR2: Self = Self(24);
    pub const STAR3: Self = Self(25);
    pub const STAR4: Self = Self(26);
    pub const NUM2: Self = Self(27);
    pub const NUM3: Self = Self(28);
    pub const NUM4: Self = Self(29);
    pub const NUM5: Self = Self(30);
    pub const NUM6: Self = Self(31);
    pub const NUM7: Self = Self(32);
    pub const NUM8: Self = Self(33);
    pub const NUM9: Self = Self(34);
    pub const NUMA: Self = Self(35);
    pub const NUMB: Self = Self(36);
    pub const NUMC: Self = Self(37);

    // Extra vendor-defined keys X1..X26 (chart indices 38-63)
    pub const X1: Self = Self(38);
    pub const X2: Self = Self(39);
    pub const X3: Self = Self(40);
    pub const X4: Self = Self(41);
    pub const X5: Self = Self(42);
    pub const X6: Self = Self(43);
    pub const X7: Self = Self(44);
    pub const X8: Self = Self(45);
    pub const X9: Self = Self(46);
    pub const X10: Self = Self(47);
    pub const X11: Self = Self(48);
    pub const X12: Self = Self(49);
    pub const X13: Self = Self(50);
    pub const X14: Self = Self(51);
    pub const X15: Self = Self(52);
    pub const X16: Self = Self(53);
    pub const X17: Self = Self(54);
    pub const X18: Self = Self(55);
    pub const X19: Self = Self(56);
    pub const X20: Self = Self(57);
    pub const X21: Self = Self(58);
    pub const X22: Self = Self(59);
    pub const X23: Self = Self(60);
    pub const X24: Self = Self(61);
    pub const X25: Self = Self(62);
    pub const X26: Self = Self(63);

    /// Alias for the most commonly used asterisk bit.
    pub const STAR: Self = Self::STAR1;

    /// Returns the chart index (0..=63) of this key.
    #[inline]
    pub const fn chart_index(self) -> u8 {
        self.0
    }
}
