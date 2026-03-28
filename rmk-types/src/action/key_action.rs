//! Composite key actions stored in the keymap.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::{Action, MorseProfile};

/// A KeyAction is the action at a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like tap/hold
#[derive(Debug, Copy, Clone, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
pub enum KeyAction {
    /// No action
    No,
    /// Transparent action, next layer will be checked
    Transparent,
    /// A single action, such as triggering a key, or activating a layer. Action is triggered when pressed and cancelled when released.
    Single(Action),
    /// Don't wait the release of the key, auto-release after a time threshold.
    Tap(Action),
    /// Tap hold action
    TapHold(Action, Action, MorseProfile),

    /// Morse action, references a morse configuration by index.
    Morse(u8),
}

impl KeyAction {
    /// Convert `KeyAction` to the internal `Action`.
    /// Only valid for `Single` and `Tap` variant, returns `Action::No` for other variants.
    pub fn to_action(self) -> Action {
        match self {
            KeyAction::Single(a) | KeyAction::Tap(a) => a,
            _ => Action::No,
        }
    }

    /// 'morse' is an alias for the superset of tap dance and tap hold keys,
    /// since their handling have many similarities
    pub fn is_morse(&self) -> bool {
        matches!(self, KeyAction::TapHold(_, _, _) | KeyAction::Morse(_))
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, KeyAction::No)
    }
}

/// combo, fork, etc. compares key actions
/// WARNING: this is not a perfect comparison, we ignores the profile config of TapHold!
impl PartialEq for KeyAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (KeyAction::No, KeyAction::No) => true,
            (KeyAction::Transparent, KeyAction::Transparent) => true,
            (KeyAction::Single(a), KeyAction::Single(b)) => a == b,
            (KeyAction::Tap(a), KeyAction::Tap(b)) => a == b,
            (KeyAction::TapHold(a, b, _), KeyAction::TapHold(c, d, _)) => a == c && b == d,
            (KeyAction::Morse(a), KeyAction::Morse(b)) => a == b,
            _ => false,
        }
    }
}
