//! Plover HID stenography live-state reporter.
//!
//! The Plover HID protocol sends the full bitmap of currently-held steno
//! keys on every state change (press or release). Chord detection happens
//! on the host side: Plover accumulates pressed keys and fires the chord
//! according to its own policy (all-up by default, or first-up when
//! configured).
//!
//! Wire-format ordering matches the Plover HID spec: chart index
//! 0 (`StenoKey::S1`) is the most significant bit of byte 1 of the report,
//! chart index 63 (`StenoKey::X26`) is the least significant bit of byte 8.

use rmk_types::steno::StenoKey;

use crate::hid::{Report, StenoReport};

#[derive(Debug, Default)]
pub(crate) struct StenoChord {
    /// Live bitmap of currently-held steno keys. Stored such that
    /// `state.to_be_bytes()` already has the wire-format bit order:
    /// chart index 0 = MSB of the first byte.
    state: u64,
}

impl StenoChord {
    pub(crate) const fn new() -> Self {
        Self { state: 0 }
    }

    /// Update live state for a steno key event. Returns `true` only when the
    /// bitmap actually changed, so callers can skip the report dispatch on
    /// no-op edges (re-press of a held key, re-release of an already-released
    /// key, or out-of-range chart indices).
    pub(crate) fn update(&mut self, key: StenoKey, pressed: bool) -> bool {
        let Some(mask) = key.bit_mask() else { return false };
        let prev = self.state;
        if pressed {
            self.state |= mask;
        } else {
            self.state &= !mask;
        }
        prev != self.state
    }

    /// Build a report reflecting the current bitmap of held steno keys.
    pub(crate) fn current_report(&self) -> Report {
        Report::StenoReport(StenoReport {
            keys: self.state.to_be_bytes(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_bytes(s: &StenoChord) -> [u8; 8] {
        match s.current_report() {
            Report::StenoReport(r) => r.keys,
            _ => panic!("expected StenoReport"),
        }
    }

    fn bitmap(s: &StenoChord) -> u64 {
        u64::from_be_bytes(report_bytes(s))
    }

    fn msb_bit(key: StenoKey) -> u64 {
        key.bit_mask().expect("test key must be in range")
    }

    #[test]
    fn press_sends_live_state() {
        let mut s = StenoChord::new();
        s.update(StenoKey::S1, true);
        assert_eq!(bitmap(&s), msb_bit(StenoKey::S1));

        s.update(StenoKey::T, true);
        assert_eq!(bitmap(&s), msb_bit(StenoKey::S1) | msb_bit(StenoKey::T));
    }

    #[test]
    fn release_clears_bit() {
        let mut s = StenoChord::new();
        s.update(StenoKey::S1, true);
        s.update(StenoKey::T, true);

        s.update(StenoKey::T, false);
        assert_eq!(bitmap(&s), msb_bit(StenoKey::S1), "T bit should be cleared");

        s.update(StenoKey::S1, false);
        assert_eq!(bitmap(&s), 0, "all bits should be cleared");
    }

    #[test]
    fn real_edges_signal_change() {
        let mut s = StenoChord::new();
        assert!(s.update(StenoKey::S1, true));
        assert!(s.update(StenoKey::T, true));
        assert!(s.update(StenoKey::T, false));
        assert!(s.update(StenoKey::S1, false));
    }

    #[test]
    fn no_op_edges_skip_dispatch() {
        let mut s = StenoChord::new();
        assert!(s.update(StenoKey::S1, true));
        assert!(!s.update(StenoKey::S1, true), "re-press of held key is a no-op");
        assert!(s.update(StenoKey::S1, false));
        assert!(!s.update(StenoKey::S1, false), "re-release of cleared key is a no-op");
        assert!(!s.update(StenoKey::T, false), "release of never-pressed key is a no-op");
    }

    #[test]
    fn s1_is_msb_of_first_byte() {
        let mut s = StenoChord::new();
        s.update(StenoKey::S1, true);
        let bytes = report_bytes(&s);
        assert_eq!(bytes[0], 0x80);
        assert_eq!(&bytes[1..], &[0; 7]);
    }

    #[test]
    fn x26_is_lsb_of_last_byte() {
        let mut s = StenoChord::new();
        s.update(StenoKey::X26, true);
        let bytes = report_bytes(&s);
        assert_eq!(&bytes[..7], &[0; 7]);
        assert_eq!(bytes[7], 0x01);
    }

    #[test]
    fn out_of_range_index_is_skipped() {
        let mut s = StenoChord::new();
        assert!(!s.update(StenoKey(64), true));
    }

    #[test]
    fn repeated_chord_produces_zero_between() {
        let mut s = StenoChord::new();
        for _ in 0..3 {
            s.update(StenoKey::S1, true);
            s.update(StenoKey::T, true);
            s.update(StenoKey::T, false);
            s.update(StenoKey::S1, false);
            assert_eq!(bitmap(&s), 0, "all-up must produce zero bitmap");
        }
    }

    /// Press all keys, release all keys, return the peak bitmap held during
    /// the stroke (when all keys were pressed).
    fn stroke(s: &mut StenoChord, keys: &[StenoKey]) -> u64 {
        let mut peak = 0u64;
        for &k in keys {
            s.update(k, true);
            peak = bitmap(s);
        }
        for &k in keys {
            s.update(k, false);
        }
        peak
    }

    /// "hello world" in Plover's default dictionary is three strokes:
    ///   HEL  (H + E + -L)
    ///   HRO  (H + R + O)
    ///   WORLD (W + O + -R + -L + -D)
    #[test]
    fn hello_world_strokes() {
        let mut s = StenoChord::new();

        let hel = stroke(&mut s, &[StenoKey::H, StenoKey::RE, StenoKey::RL]);
        assert_eq!(
            hel,
            msb_bit(StenoKey::H) | msb_bit(StenoKey::RE) | msb_bit(StenoKey::RL)
        );

        let hro = stroke(&mut s, &[StenoKey::H, StenoKey::R, StenoKey::O]);
        assert_eq!(hro, msb_bit(StenoKey::H) | msb_bit(StenoKey::R) | msb_bit(StenoKey::O));

        let world = stroke(
            &mut s,
            &[StenoKey::W, StenoKey::O, StenoKey::RR, StenoKey::RL, StenoKey::RD],
        );
        assert_eq!(
            world,
            msb_bit(StenoKey::W)
                | msb_bit(StenoKey::O)
                | msb_bit(StenoKey::RR)
                | msb_bit(StenoKey::RL)
                | msb_bit(StenoKey::RD)
        );
    }
}
