//! Morse key types shared between firmware and protocol layers.
//!
//! A morse key behaves differently according to the pattern of a tap/hold sequence.
//! The maximum number of taps is limited to 15 by the internal u16 representation
//! of [`MorsePattern`].

use heapless::LinearMap;
use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::schema::{DataModelType, NamedType, NamedValue};
use serde::{Deserialize, Serialize};

use crate::action::{Action, MorseProfile};

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

    pub fn from_u16(value: u16) -> Self {
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
        15 - self.0.leading_zeros() as usize
    }

    /// Checks if this pattern starts with the given one
    pub fn starts_with(&self, pattern_start: MorsePattern) -> bool {
        let n = pattern_start.0.leading_zeros();
        let m = self.0.leading_zeros();
        m <= n && (self.0 >> (n - m) == pattern_start.0)
    }

    pub fn last_is_hold(&self) -> bool {
        self.0 & 0b1 == 0b1
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
/// The number of pairs is limited by `NUM_PATTERNS`, which is a const generic parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Morse<const NUM_PATTERNS: usize> {
    /// The profile of this morse key, which defines the timing parameters, etc.
    /// If some of its fields are filled with None, the global default value will be used.
    pub profile: MorseProfile,
    /// The list of pattern -> action pairs, which can be triggered
    #[serde(with = "morse_actions_serde")]
    pub actions: LinearMap<MorsePattern, Action, NUM_PATTERNS>,
}

impl<const NUM_PATTERNS: usize> MaxSize for Morse<NUM_PATTERNS> {
    const POSTCARD_MAX_SIZE: usize = MorseProfile::POSTCARD_MAX_SIZE
        + (<(u16, Action)>::POSTCARD_MAX_SIZE) * NUM_PATTERNS
        + crate::varint_max_size(NUM_PATTERNS);
}

#[cfg(feature = "defmt")]
impl<const NUM_PATTERNS: usize> defmt::Format for Morse<NUM_PATTERNS> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "profile: MorseProfile({:?}), ", self.profile);
        defmt::write!(f, "actions: [");
        for item in self.actions.iter() {
            defmt::write!(f, "{:?},", item);
        }
        defmt::write!(f, "]");
    }
}

impl<const NUM_PATTERNS: usize> PartialEq for Morse<NUM_PATTERNS> {
    fn eq(&self, other: &Self) -> bool {
        if self.profile != other.profile || self.actions.len() != other.actions.len() {
            return false;
        }
        self.actions.iter().all(|(k, v)| other.actions.get(k) == Some(v))
    }
}

impl<const NUM_PATTERNS: usize> Eq for Morse<NUM_PATTERNS> {}

/// Manual Schema impl because Morse uses custom serde for LinearMap.
/// The wire format is: (MorseProfile, Vec<(u16, Action)>).
#[cfg(feature = "rmk_protocol")]
impl<const NUM_PATTERNS: usize> Schema for Morse<NUM_PATTERNS> {
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
                    name: "Vec<(u16, Action)>",
                    ty: &DataModelType::Seq(&NamedType {
                        name: "(u16, Action)",
                        ty: &DataModelType::Tuple(&[<u16 as Schema>::SCHEMA, <Action as Schema>::SCHEMA]),
                    }),
                },
            },
        ]),
    };
}

impl<const N: usize> Default for Morse<N> {
    fn default() -> Self {
        Self {
            profile: MorseProfile::const_default(),
            actions: LinearMap::default(),
        }
    }
}

impl<const N: usize> Morse<N> {
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
        tap_actions: heapless::Vec<Action, N>,
        hold_actions: heapless::Vec<Action, N>,
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
    pub fn put(
        &mut self,
        pattern: MorsePattern,
        action: Action,
    ) -> Result<(), (MorsePattern, Action)> {
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

    pub fn serialize<S, const N: usize>(
        map: &LinearMap<MorsePattern, Action, N>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert to Vec for serialization
        let vec: heapless::Vec<(u16, Action), N> = map.iter().map(|(k, v)| (k.to_u16(), *v)).collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<LinearMap<MorsePattern, Action, N>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use core::fmt;

        use serde::de::{SeqAccess, Visitor};

        struct VecVisitor<const N: usize>;

        impl<'de, const N: usize> Visitor<'de> for VecVisitor<N> {
            type Value = heapless::Vec<(u16, Action), N>;

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

        let vec = deserializer.deserialize_seq(VecVisitor::<N>)?;
        let mut map = LinearMap::new();
        for (pattern, action) in vec {
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
    use crate::action::{Action, MorseMode, MorseProfile};
    use crate::keycode::{HidKeyCode, KeyCode};

    #[test]
    fn test_linear_map_serde_empty() {
        let morse = Morse::<4>::default();

        let mut buffer = [0u8; 128];
        let serialized = postcard::to_slice(&morse, &mut buffer).unwrap();
        let deserialized: Morse<4> = postcard::from_bytes(serialized).unwrap();

        assert_eq!(morse.actions.len(), deserialized.actions.len());
        assert_eq!(morse.actions.len(), 0);
    }

    #[test]
    fn test_linear_map_serde_single_entry() {
        let mut morse = Morse::<4>::default();
        morse.actions.insert(TAP, Action::Key(KeyCode::Hid(HidKeyCode::A))).ok();

        let mut buffer = [0u8; 128];
        let serialized = postcard::to_slice(&morse, &mut buffer).unwrap();
        let deserialized: Morse<4> = postcard::from_bytes(serialized).unwrap();

        assert_eq!(morse.actions.len(), deserialized.actions.len());
        assert_eq!(
            deserialized.actions.get(&TAP),
            Some(&Action::Key(KeyCode::Hid(HidKeyCode::A)))
        );
    }

    #[test]
    fn test_linear_map_serde_multiple_entries() {
        let mut morse = Morse::<4>::default();
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
        let deserialized: Morse<4> = postcard::from_bytes(serialized).unwrap();

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
        let mut morse = Morse::<4> {
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
        let deserialized: Morse<4> = postcard::from_bytes(serialized).unwrap();

        assert_eq!(morse.profile, deserialized.profile);
        assert_eq!(morse.actions.len(), deserialized.actions.len());
    }
}
