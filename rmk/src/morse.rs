use crate::action::Action;
use crate::keycode::ModifierCombination;

/// Definition of a morse key.
///
/// A morse key is a key that behaves differently according to the number of taps and current action.
/// It's morse code, but supports only holding after a certain number of taps.
///
/// There are two lists of actions in a morse key:
/// - tap actions: actions triggered by tapping the key n times
/// - hold actions: actions triggered by tapping the key n times then holding the key
///
/// The maximum number of taps is defined by the `TAP_N` parameter.
///
/// The morse key is actually a superset of tap-hold key and tap-dance key.
/// When `TAP_N` is 1, the morse key becomes a tap-hold key, and when `hold_actions` is empty, it becomes a tap-dance key.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Morse<const TAP_N: usize> {
    /// The actions triggered by tapping the key
    pub(crate) tap_actions: MorseActions<TAP_N>,
    /// The actions triggered by tapping and holding the key
    pub(crate) hold_actions: MorseActions<TAP_N>,
    /// The timeout time for each operation in milliseconds
    pub(crate) timeout_ms: u16,
    /// The decision mode of the morse key
    pub mode: MorseKeyMode,
    /// If the unilateral tap is enabled
    pub unilateral_tap: bool,
}

impl<const TAP_N: usize> Default for Morse<TAP_N> {
    fn default() -> Self {
        Self {
            tap_actions: MorseActions::default(),
            hold_actions: MorseActions::default(),
            timeout_ms: 250,
            mode: MorseKeyMode::Normal,
            unilateral_tap: false,
        }
    }
}

impl<const TAP_N: usize> Morse<TAP_N> {
    pub const fn new_tap_hold(tap_action: Action, hold_action: Action) -> Self {
        let tap_actions = MorseActions::new_single(tap_action);
        let hold_actions = MorseActions::new_single(hold_action);
        Self {
            tap_actions,
            hold_actions,
            timeout_ms: 250,
            mode: MorseKeyMode::Normal,
            unilateral_tap: false,
        }
    }

    pub const fn new_layer_tap_hold(tap_action: Action, layer: u8) -> Self {
        let tap_actions = MorseActions::new_single(tap_action);
        let hold_actions = MorseActions::new_single(Action::LayerOn(layer));
        Self {
            tap_actions,
            hold_actions,
            timeout_ms: 250,
            mode: MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }

    pub const fn new_modifier_tap_hold(tap_action: Action, modifier: ModifierCombination) -> Self {
        let tap_actions = MorseActions::new_single(tap_action);
        let hold_actions = MorseActions::new_single(Action::Modifier(modifier));
        Self {
            tap_actions,
            hold_actions,
            timeout_ms: 250,
            mode: MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }

    pub const fn new_hrm(tap_action: Action, modifier: ModifierCombination) -> Self {
        let tap_actions = MorseActions::new_single(tap_action);
        let hold_actions = MorseActions::new_single(Action::Modifier(modifier));
        Self {
            tap_actions,
            hold_actions,
            timeout_ms: 250,
            mode: MorseKeyMode::PermissiveHold,
            unilateral_tap: true,
        }
    }

    pub fn new_tap_dance(tap_action: [Action; TAP_N], hold_action: [Action; TAP_N], timeout_ms: u16) -> Self {
        let tap_actions = MorseActions::new(tap_action);
        let hold_actions = MorseActions::new(hold_action);
        Self {
            tap_actions,
            hold_actions,
            timeout_ms,
            mode: MorseKeyMode::HoldOnOtherPress,
            unilateral_tap: false,
        }
    }

    pub const fn new_tap_hold_with_config(
        tap_action: Action,
        hold_action: Action,
        timeout_ms: u16,
        mode: MorseKeyMode,
        unilateral_tap: bool,
    ) -> Self {
        let tap_actions = MorseActions::new_single(tap_action);
        let hold_actions = MorseActions::new_single(hold_action);
        Self {
            tap_actions,
            hold_actions,
            timeout_ms,
            mode,
            unilateral_tap,
        }
    }

    // TODO: Remove the global setting
    pub fn get_timeout(&self, global_timeout_time: u16) -> u16 {
        if self.timeout_ms == 250 && global_timeout_time != 250 {
            // Global setting overrides the default setting
            global_timeout_time
        } else {
            self.timeout_ms
        }
    }

    pub fn tap_action(&self, index: usize) -> Action {
        *self.tap_actions.get(index).unwrap_or(&Action::No)
    }

    pub fn hold_action(&self, index: usize) -> Action {
        *self.hold_actions.get(index).unwrap_or(&Action::No)
    }
}

/// Mode for morse key behavior
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MorseKeyMode {
    /// Normal mode, the decision is made when timeout
    Normal,
    /// Same as QMK's permissive hold: https://docs.qmk.fm/tap_hold#tap-or-hold-decision-modes
    /// When another key is pressed and released during the current morse key is held,
    /// the hold action of current morse key will be triggered
    PermissiveHold,
    /// Trigger hold immediately if any other non-morse key is pressed when the current morse key is held
    HoldOnOtherPress,
}

/// The list of actions for a morse key.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorseActions<const N: usize> {
    /// The actions list, use `Action::No` to represent an empty slot
    actions: [Action; N],
    /// The number of saved actions
    len: u8,
}

impl<const N: usize> Default for MorseActions<N> {
    fn default() -> Self {
        Self {
            actions: [Action::No; N],
            len: 0,
        }
    }
}

impl<const N: usize> MorseActions<N> {
    pub fn empty() -> Self {
        Self {
            actions: [Action::No; N],
            len: 0,
        }
    }

    pub fn new(actions: [Action; N]) -> Self {
        let mut len = 0;
        for action in actions {
            if action != Action::No {
                len += 1;
            }
        }
        Self { actions, len }
    }

    pub const fn new_from_list(actions: [Action; N], len: u8) -> Self {
        Self { actions, len }
    }

    pub const fn new_single(action: Action) -> Self {
        Self {
            actions: [action; N],
            len: 1,
        }
    }

    pub fn push(&mut self, action: Action) {
        if self.len < N as u8 {
            // Find first empty slot
            for i in 0..N {
                if self.actions[i] == Action::No {
                    self.actions[i] = action;
                    self.len += 1;
                    break;
                }
            }
        } else {
            warn!("MorseAction list is full");
        }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn get(&self, index: usize) -> Option<&Action> {
        self.actions.get(index)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
