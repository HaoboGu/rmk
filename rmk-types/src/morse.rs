//! Morse key types shared between firmware and protocol layers.
//!
//! This module contains all morse-related types:
//! - [`MorseMode`] / [`MorseProfile`] — timing and behavior configuration
//! - [`MorsePattern`] — tap/hold pattern encoding (up to 15 steps in a u16)
//! - [`Morse`] — full morse key definition (profile + pattern→action map)

use heapless::LinearMap;
use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::schema::{DataModelType, NamedType, NamedValue};
use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::constants::MORSE_SIZE;

// ---------------------------------------------------------------------------
// MorseMode & MorseProfile — timing/behavior configuration
// ---------------------------------------------------------------------------

/// Mode for morse key behavior
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[repr(u8)]
pub enum MorseMode {
    /// Same as QMK's permissive hold: https://docs.qmk.fm/tap_hold#tap-or-hold-decision-modes
    /// When another key is pressed and released during the current morse key is held,
    /// the hold action of current morse key will be triggered
    PermissiveHold,
    /// Trigger hold immediately if any other non-morse key is pressed when the current morse key is held
    HoldOnOtherPress,
    /// Normal mode, the decision is made when timeout
    Normal,
}

/// Configuration for morse, tap dance and tap-hold.
/// Manually packed into 32 bits to save RAM.
///
/// Bit layout of the inner `u32`:
/// ```text
/// 31  30 | 29      16 | 15  14 | 13       0
/// mode   | gap_timeout | uni_tap| hold_timeout
///  (2b)  |   (14b ms)  |  (2b)  |  (14b ms)
/// ```
///
/// - `mode` (bits 31-30): `00` = None, `01` = PermissiveHold, `10` = HoldOnOtherPress, `11` = Normal
/// - `gap_timeout` (bits 29-16): gap timeout in ms (0 = None, max 16383)
/// - `uni_tap` (bits 15-14): `00`/`01` = None, `10` = Some(false), `11` = Some(true)
/// - `hold_timeout` (bits 13-0): hold timeout in ms (0 = None, max 16383)
#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
pub struct MorseProfile(u32);

impl MorseProfile {
    pub const fn const_default() -> Self {
        Self(0)
    }

    /// If the previous key is on the same "hand", the current key will be determined as a tap
    pub fn unilateral_tap(self) -> Option<bool> {
        match self.0 & 0x0000_C000 {
            0x0000_C000 => Some(true),
            0x0000_8000 => Some(false),
            _ => None,
        }
    }

    pub const fn with_unilateral_tap(self, b: Option<bool>) -> Self {
        Self(
            (self.0 & 0xFFFF_3FFF)
                | match b {
                    Some(true) => 0x0000_C000,
                    Some(false) => 0x0000_8000,
                    None => 0,
                },
        )
    }

    /// The decision mode of the morse/tap-hold key
    pub fn mode(self) -> Option<MorseMode> {
        match self.0 & 0xC000_0000 {
            0xC000_0000 => Some(MorseMode::Normal),
            0x8000_0000 => Some(MorseMode::HoldOnOtherPress),
            0x4000_0000 => Some(MorseMode::PermissiveHold),
            _ => None,
        }
    }

    pub const fn with_mode(self, m: Option<MorseMode>) -> Self {
        Self(
            (self.0 & 0x3FFF_FFFF)
                | match m {
                    Some(MorseMode::Normal) => 0xC000_0000,
                    Some(MorseMode::HoldOnOtherPress) => 0x8000_0000,
                    Some(MorseMode::PermissiveHold) => 0x4000_0000,
                    None => 0,
                },
        )
    }

    /// If the key is pressed longer than this, it is accepted as `hold` (in milliseconds)
    pub fn hold_timeout_ms(self) -> Option<u16> {
        let t = (self.0 & 0x3FFF) as u16;
        if t == 0 { None } else { Some(t) }
    }

    pub const fn with_hold_timeout_ms(self, t: Option<u16>) -> Self {
        if let Some(t) = t {
            Self((self.0 & 0xFFFF_C000) | (t as u32 & 0x3FFF))
        } else {
            Self(self.0 & 0xFFFF_C000)
        }
    }

    pub const fn set_hold_timeout_ms(&mut self, t: u16) {
        self.0 = (self.0 & 0xFFFF_C000) | (t as u32 & 0x3FFF)
    }

    pub const fn set_gap_timeout_ms(&mut self, t: u16) {
        self.0 = (self.0 & 0xC000_FFFF) | ((t as u32 & 0x3FFF) << 16)
    }

    /// The time elapsed from the last release of a key is longer than this, it will break the morse pattern (in milliseconds)
    pub fn gap_timeout_ms(self) -> Option<u16> {
        let t = ((self.0 >> 16) & 0x3FFF) as u16;
        if t == 0 { None } else { Some(t) }
    }

    pub const fn with_gap_timeout_ms(self, t: Option<u16>) -> Self {
        if let Some(t) = t {
            Self((self.0 & 0xC000_FFFF) | ((t as u32 & 0x3FFF) << 16))
        } else {
            Self(self.0 & 0xC000_FFFF)
        }
    }

    pub const fn new(
        unilateral_tap: Option<bool>,
        mode: Option<MorseMode>,
        hold_timeout_ms: Option<u16>,
        gap_timeout_ms: Option<u16>,
    ) -> Self {
        let mut v = 0u32;
        if let Some(t) = hold_timeout_ms {
            v = (t & 0x3FFF) as u32;
        }
        if let Some(t) = gap_timeout_ms {
            v |= ((t & 0x3FFF) as u32) << 16;
        }
        if let Some(b) = unilateral_tap {
            v |= if b { 0x0000_C000 } else { 0x0000_8000 };
        }
        if let Some(m) = mode {
            v |= match m {
                MorseMode::Normal => 0xC000_0000,
                MorseMode::HoldOnOtherPress => 0x8000_0000,
                MorseMode::PermissiveHold => 0x4000_0000,
            };
        }
        MorseProfile(v)
    }
}

impl Default for MorseProfile {
    fn default() -> Self {
        MorseProfile::const_default()
    }
}

impl From<u32> for MorseProfile {
    fn from(v: u32) -> Self {
        MorseProfile(v)
    }
}

impl From<MorseProfile> for u32 {
    fn from(val: MorseProfile) -> Self {
        val.0
    }
}

// ---------------------------------------------------------------------------
// MorsePattern & Morse — pattern encoding and key definition
// ---------------------------------------------------------------------------

/// MorsePattern is a sequence of maximum 15 taps or holds that can be encoded into an u16:
/// 0x1 when empty, then 0 for tap or 1 for hold shifted from the right
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorsePattern(u16);

pub const TAP: MorsePattern = MorsePattern(0b10);
pub const HOLD: MorsePattern = MorsePattern(0b11);
pub const DOUBLE_TAP: MorsePattern = MorsePattern(0b100);
pub const HOLD_AFTER_TAP: MorsePattern = MorsePattern(0b101);

impl Default for MorsePattern {
    fn default() -> Self {
        MorsePattern(0b1) // 0b1 means empty
    }
}

impl MorsePattern {
    pub fn max_taps() -> usize {
        15 // 15 taps can be encoded on u16 bits (1 bit used to mark the start position)
    }

    /// Creates a `MorsePattern` from a raw `u16`.
    ///
    /// # Panics (debug only)
    /// Panics if `value` is 0, which is not a valid encoding
    /// (the empty pattern is `0b1`).
    pub fn from_u16(value: u16) -> Self {
        debug_assert!(value != 0, "MorsePattern 0 is invalid; the empty pattern is 0b1");
        MorsePattern(value)
    }

    pub fn to_u16(&self) -> u16 {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0b1
    }

    pub fn is_full(&self) -> bool {
        (self.0 & 0b1000_0000_0000_0000) != 0
    }

    pub fn pattern_length(&self) -> usize {
        // leading_zeros() is 16 for 0, which would underflow.
        // Saturate to 0 for the (invalid) zero case.
        15usize.saturating_sub(self.0.leading_zeros() as usize)
    }

    /// Checks if this pattern starts with the given one
    pub fn starts_with(&self, pattern_start: MorsePattern) -> bool {
        let n = pattern_start.0.leading_zeros();
        let m = self.0.leading_zeros();
        m <= n && (self.0 >> (n - m) == pattern_start.0)
    }

    /// Returns `true` if the last step in the pattern is a hold.
    /// Returns `false` for empty patterns.
    pub fn last_is_hold(&self) -> bool {
        !self.is_empty() && self.0 & 0b1 == 0b1
    }

    pub fn followed_by_tap(&self) -> Self {
        // Shift the bits to the left and set the last bit to 0 (tap)
        MorsePattern(self.0 << 1)
    }

    pub fn followed_by_hold(&self) -> Self {
        // Shift the bits to the left and set the last bit to 1 (hold)
        MorsePattern((self.0 << 1) | 0b1)
    }
}

/// Definition of a morse key.
///
/// A morse key is a key that behaves differently according to the pattern of a tap/hold sequence.
/// The maximum number of taps is limited to 15 by the internal u16 representation of MorsePattern.
/// There is a list of (pattern, corresponding action) pairs for each morse key:
/// The number of pairs is limited by `MORSE_SIZE` (from `constants.rs`, generated at build time).
///
/// Note: `MORSE_SIZE` is a **wire-format** capacity — on firmware it equals
/// `MAX_PATTERNS_PER_KEY` (from `keyboard.toml`), on host it's a fixed upper bound.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Morse {
    /// The profile of this morse key, which defines the timing parameters, etc.
    /// If some of its fields are filled with None, the global default value will be used.
    pub profile: MorseProfile,
    /// The list of pattern -> action pairs, which can be triggered
    #[serde(with = "morse_actions_serde")]
    pub actions: LinearMap<MorsePattern, Action, MORSE_SIZE>,
}

impl MaxSize for Morse {
    // The custom serializer in `morse_actions_serde` (below) emits the
    // `LinearMap` as `Vec<(u16, Action), MORSE_SIZE>` on the wire — keep that
    // shape in sync with the helper type parameter here.
    const POSTCARD_MAX_SIZE: usize =
        MorseProfile::POSTCARD_MAX_SIZE + crate::heapless_vec_max_size::<(u16, Action), MORSE_SIZE>();
}

#[cfg(feature = "defmt")]
impl defmt::Format for Morse {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "profile: MorseProfile({:?}), ", self.profile);
        defmt::write!(f, "actions: [");
        for item in self.actions.iter() {
            defmt::write!(f, "{:?},", item);
        }
        defmt::write!(f, "]");
    }
}

impl PartialEq for Morse {
    fn eq(&self, other: &Self) -> bool {
        if self.profile != other.profile || self.actions.len() != other.actions.len() {
            return false;
        }
        self.actions.iter().all(|(k, v)| other.actions.get(k) == Some(v))
    }
}

impl Eq for Morse {}

/// Manual Schema impl because Morse uses custom serde for LinearMap.
/// The wire format is: (MorseProfile, Vec<(u16, Action)>).
///
/// **Important:** This must stay in sync with the custom serde impl in
/// `morse_actions_serde`. If the wire format changes, update this Schema
/// accordingly. The `morse_schema_matches_wire_format` test validates
/// this invariant.
#[cfg(feature = "rmk_protocol")]
impl Schema for Morse {
    const SCHEMA: &'static NamedType = &NamedType {
        name: "Morse",
        ty: &DataModelType::Struct(&[
            &NamedValue {
                name: "profile",
                ty: <MorseProfile as Schema>::SCHEMA,
            },
            &NamedValue {
                name: "actions",
                ty: &NamedType {
                    name: "MorseActions",
                    ty: &DataModelType::Seq(&NamedType {
                        name: "MorseActionEntry",
                        ty: &DataModelType::Tuple(&[<u16 as Schema>::SCHEMA, <Action as Schema>::SCHEMA]),
                    }),
                },
            },
        ]),
    };
}

impl Morse {
    pub fn new_from_vial(
        tap: Action,
        hold: Action,
        hold_after_tap: Action,
        double_tap: Action,
        profile: MorseProfile,
    ) -> Self {
        let mut result = Self {
            profile,
            ..Default::default()
        };

        if tap != Action::No {
            _ = result.actions.insert(TAP, tap);
        }
        if hold != Action::No {
            _ = result.actions.insert(HOLD, hold);
        }
        if double_tap != Action::No {
            _ = result.actions.insert(DOUBLE_TAP, double_tap);
        }
        if hold_after_tap != Action::No {
            _ = result.actions.insert(HOLD_AFTER_TAP, hold_after_tap);
        }
        result
    }

    pub fn new_with_actions(
        tap_actions: heapless::Vec<Action, MORSE_SIZE>,
        hold_actions: heapless::Vec<Action, MORSE_SIZE>,
        profile: MorseProfile,
    ) -> Self {
        let mut result = Self {
            profile,
            ..Default::default()
        };

        let mut pattern = 0b1u16;
        for item in tap_actions.iter() {
            pattern <<= 1;
            let _ = result.put(MorsePattern::from_u16(pattern), *item);
        }

        let mut pattern = 0b1u16;
        for item in hold_actions.iter() {
            pattern <<= 1;
            let _ = result.put(MorsePattern::from_u16(pattern | 0b1), *item);
        }

        result
    }

    pub fn max_pattern_length(&self) -> usize {
        let mut max_length = 0;
        for pair in self.actions.iter() {
            max_length = max_length.max(pair.0.pattern_length());
        }
        max_length
    }

    pub fn try_predict_final_action(&self, pattern_start: MorsePattern) -> Option<Action> {
        if !self.actions.contains_key(&pattern_start) {
            return None;
        }
        for (pattern, _) in self.actions.iter() {
            if *pattern != pattern_start && pattern.starts_with(pattern_start) {
                return None;
            }
        }
        self.actions.get(&pattern_start).copied()
    }

    pub fn can_fire_early(&self, pattern: MorsePattern) -> bool {
        let Some(current_action) = self.actions.get(&pattern) else {
            return false;
        };
        if self.actions.contains_key(&pattern.followed_by_tap()) {
            return false;
        }
        self.actions
            .get(&pattern.followed_by_hold())
            .is_some_and(|a| *a == *current_action)
    }

    pub fn has_pattern_or_continuation(&self, pattern: MorsePattern) -> bool {
        self.actions.iter().any(|(p, _)| p.starts_with(pattern))
    }

    pub fn get(&self, pattern: MorsePattern) -> Option<Action> {
        self.actions.get(&pattern).copied()
    }

    /// Insert or update an action for the given pattern.
    ///
    /// An `Action::No` removes the pattern. Returns `Err((pattern, action))` if the map is full.
    pub fn put(&mut self, pattern: MorsePattern, action: Action) -> Result<(), (MorsePattern, Action)> {
        if action != Action::No {
            self.actions.insert(pattern, action).map(|_| ())
        } else {
            let _ = self.actions.remove(&pattern);
            Ok(())
        }
    }
}

// Custom serde module for LinearMap
mod morse_actions_serde {
    use serde::de::Error;
    use serde::{Deserializer, Serializer};

    use super::*;

    pub fn serialize<S>(map: &LinearMap<MorsePattern, Action, MORSE_SIZE>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert to Vec for serialization
        let vec: heapless::Vec<(u16, Action), MORSE_SIZE> = map.iter().map(|(k, v)| (k.to_u16(), *v)).collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<LinearMap<MorsePattern, Action, MORSE_SIZE>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::fmt;

        use serde::de::{SeqAccess, Visitor};

        struct VecVisitor;

        impl<'de> Visitor<'de> for VecVisitor {
            type Value = heapless::Vec<(u16, Action), MORSE_SIZE>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a sequence of (u16, Action) tuples")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut vec = heapless::Vec::new();
                while let Some(elem) = seq.next_element::<(u16, Action)>()? {
                    vec.push(elem)
                        .map_err(|_| serde::de::Error::custom("Vec capacity exceeded"))?;
                }
                Ok(vec)
            }
        }

        let vec = deserializer.deserialize_seq(VecVisitor)?;
        let mut map = LinearMap::new();
        for (pattern, action) in vec {
            if pattern == 0 {
                return Err(D::Error::custom("MorsePattern 0 is invalid; the empty pattern is 0b1"));
            }
            map.insert(MorsePattern::from_u16(pattern), action)
                .map_err(|_| D::Error::custom("Failed to insert into LinearMap"))?;
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use crate::action::Action;
    use crate::keycode::{HidKeyCode, KeyCode};

    #[test]
    fn test_linear_map_serde_empty() {
        let morse = Morse::default();

        let mut buffer = [0u8; 128];
        let serialized = postcard::to_slice(&morse, &mut buffer).unwrap();
        let deserialized: Morse = postcard::from_bytes(serialized).unwrap();

        assert_eq!(morse.actions.len(), deserialized.actions.len());
        assert_eq!(morse.actions.len(), 0);
    }

    #[test]
    fn test_linear_map_serde_single_entry() {
        let mut morse = Morse::default();
        morse.actions.insert(TAP, Action::Key(KeyCode::Hid(HidKeyCode::A))).ok();

        let mut buffer = [0u8; 128];
        let serialized = postcard::to_slice(&morse, &mut buffer).unwrap();
        let deserialized: Morse = postcard::from_bytes(serialized).unwrap();

        assert_eq!(morse.actions.len(), deserialized.actions.len());
        assert_eq!(
            deserialized.actions.get(&TAP),
            Some(&Action::Key(KeyCode::Hid(HidKeyCode::A)))
        );
    }

    #[test]
    fn test_linear_map_serde_multiple_entries() {
        let mut morse = Morse::default();
        morse.actions.insert(TAP, Action::Key(KeyCode::Hid(HidKeyCode::A))).ok();
        morse
            .actions
            .insert(HOLD, Action::Key(KeyCode::Hid(HidKeyCode::B)))
            .ok();
        morse
            .actions
            .insert(DOUBLE_TAP, Action::Key(KeyCode::Hid(HidKeyCode::C)))
            .ok();
        morse
            .actions
            .insert(HOLD_AFTER_TAP, Action::Key(KeyCode::Hid(HidKeyCode::D)))
            .ok();

        let mut buffer = [0u8; 128];
        let serialized = postcard::to_slice(&morse, &mut buffer).unwrap();
        let deserialized: Morse = postcard::from_bytes(serialized).unwrap();

        assert_eq!(morse.actions.len(), deserialized.actions.len());
        assert_eq!(morse.actions.len(), 4);

        assert_eq!(
            deserialized.actions.get(&TAP),
            Some(&Action::Key(KeyCode::Hid(HidKeyCode::A)))
        );
        assert_eq!(
            deserialized.actions.get(&HOLD),
            Some(&Action::Key(KeyCode::Hid(HidKeyCode::B)))
        );
        assert_eq!(
            deserialized.actions.get(&DOUBLE_TAP),
            Some(&Action::Key(KeyCode::Hid(HidKeyCode::C)))
        );
        assert_eq!(
            deserialized.actions.get(&HOLD_AFTER_TAP),
            Some(&Action::Key(KeyCode::Hid(HidKeyCode::D)))
        );
    }

    #[test]
    fn test_linear_map_serde_with_profile() {
        let mut morse = Morse {
            profile: MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(200), Some(150)),
            ..Default::default()
        };
        morse.actions.insert(TAP, Action::Key(KeyCode::Hid(HidKeyCode::H))).ok();
        morse
            .actions
            .insert(HOLD, Action::Key(KeyCode::Hid(HidKeyCode::I)))
            .ok();

        let mut buffer = [0u8; 128];
        let serialized = postcard::to_slice(&morse, &mut buffer).unwrap();
        let deserialized: Morse = postcard::from_bytes(serialized).unwrap();

        assert_eq!(morse.profile, deserialized.profile);
        assert_eq!(morse.actions.len(), deserialized.actions.len());
    }

    #[test]
    fn morse_pattern_max_size_matches_u16() {
        // Morse actions serialize MorsePattern as u16 on the wire.
        // If MorsePattern's MaxSize ever diverges from u16, the manual
        // MaxSize impl on Morse would be wrong.
        assert_eq!(MorsePattern::POSTCARD_MAX_SIZE, u16::POSTCARD_MAX_SIZE,);
    }

    #[test]
    fn test_morse_profile_timeout_setters() {
        let mut profile = MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(1000), Some(2000));

        assert_eq!(profile.hold_timeout_ms(), Some(1000));
        assert_eq!(profile.gap_timeout_ms(), Some(2000));
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));

        profile.set_hold_timeout_ms(1500);
        assert_eq!(profile.hold_timeout_ms(), Some(1500));
        assert_eq!(profile.gap_timeout_ms(), Some(2000));
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));

        profile.set_gap_timeout_ms(2500);
        assert_eq!(profile.hold_timeout_ms(), Some(1500));
        assert_eq!(profile.gap_timeout_ms(), Some(2500));
        assert_eq!(profile.unilateral_tap(), Some(true));
        assert_eq!(profile.mode(), Some(MorseMode::PermissiveHold));

        profile.set_hold_timeout_ms(0x3FFF);
        profile.set_gap_timeout_ms(0x3FFF);
        assert_eq!(profile.hold_timeout_ms(), Some(0x3FFF));
        assert_eq!(profile.gap_timeout_ms(), Some(0x3FFF));

        profile.set_hold_timeout_ms(0);
        profile.set_gap_timeout_ms(0);
        assert_eq!(profile.hold_timeout_ms(), None);
        assert_eq!(profile.gap_timeout_ms(), None);
    }

    /// Validates that the manual Schema impl matches the actual serde wire format.
    ///
    /// The Schema claims Morse serializes as:
    ///   struct { profile: MorseProfile, actions: Vec<(u16, Action)> }
    ///
    /// We verify this by checking that a Morse value can be reconstructed by
    /// manually deserializing its two fields in order using the same bytes.
    #[test]
    fn morse_schema_matches_wire_format() {
        use postcard::to_slice;

        // Build a Morse with known data
        let mut morse = Morse::default();
        morse.actions.insert(MorsePattern::from_u16(0b11), Action::No).unwrap();

        // Serialize the whole Morse
        let mut buf = [0u8; 256];
        let bytes = to_slice(&morse, &mut buf).unwrap();

        // Now manually deserialize field-by-field in the order the Schema declares:
        // 1. profile: MorseProfile (a newtype around u32)
        let (profile, rest): (MorseProfile, &[u8]) =
            postcard::take_from_bytes(bytes).expect("should deserialize MorseProfile first");
        assert_eq!(profile, MorseProfile::const_default());

        // 2. actions: Vec<(u16, Action)> — which is what the custom serde produces
        let (actions, rest): (heapless::Vec<(u16, Action), MORSE_SIZE>, &[u8]) =
            postcard::take_from_bytes(rest).expect("should deserialize actions vec second");
        assert!(rest.is_empty(), "no trailing bytes should remain");

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], (0b11u16, Action::No));
    }
}
