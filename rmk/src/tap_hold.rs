use embassy_time::Instant;

use crate::action::KeyAction;
use crate::event::{KeyPos, KeyboardEvent, KeyboardEventPos};

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TapHoldDecision {
    // Hold timeout, trigger the hold action
    Timeout,
    // Clean holding buffer due to permissive hold or chordal hold is triggered
    CleanBuffer,
    // Holding
    Hold,
    // Chordal holding
    ChordHold,
    // A tap hold key is release as tap
    BufferTapping,
    // Hold on other key press
    HoldOnOtherPress,
    // Skip key action processing and buffer key event
    Buffering,
    // Continue processing as normal key event
    Ignore,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HoldingKey {
    pub state: TapHoldState,
    pub event: KeyboardEvent,
    // TODO: remove it, using `Keyboard.timer` instead
    pub pressed_time: Instant,
    pub action: KeyAction,
}

impl HoldingKey {
    pub(crate) fn is_tap_hold(&self) -> bool {
        matches!(self.action, KeyAction::TapHold(_, _))
    }

    pub(crate) fn is_tap_dance(&self) -> bool {
        matches!(self.action, KeyAction::TapDance(_))
    }

    pub(crate) fn update_state(&mut self, new_state: TapHoldState) {
        self.state = new_state;
    }

    pub(crate) fn press_time(&self) -> Instant {
        self.pressed_time
    }

    pub(crate) fn state(&self) -> TapHoldState {
        self.state
    }

    pub(crate) fn tap_num(&self) -> u8 {
        match self.state {
            TapHoldState::Tap(num) => num,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TapHoldState {
    /// After a press event is received.
    /// The number represents the number of completed "tap".
    Tap(u8),
    /// Waiting for combo
    WaitingCombo,
    /// Tap key has been processed and sent to HID, but not yet released
    /// The number is used for tap-dance keys, represents the number of completed "tap"
    PostTap(u8),
    /// Tap-hold key has been determined as Hold, waiting to be processed
    BeforeHold,
    /// Key is being held, but not yet released
    /// The number is used for tap-dance keys, represents the number of completed "tap"
    PostHold(u8),
    /// Key needs to be released but is still in the queue;
    /// should be cleaned up in the main loop regardless
    Release,
    /// Idle state after tap u8 times for tap-dance keys
    IdleAfterTap(u8),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChordHoldHand {
    Left,
    Right,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChordHoldState<const COUNT: usize> {
    pub is_vertical_chord: bool,
    pub hand: ChordHoldHand,
}

impl<const COUNT: usize> ChordHoldState<COUNT> {
    // is the key event in the same side of current chord hold
    pub fn is_same_hand_key_pos(&self, key_pos: KeyPos) -> bool {
        if self.is_vertical_chord {
            return self.is_same_hand(key_pos.row as usize);
        } else {
            return self.is_same_hand(key_pos.col as usize);
        }
    }

    pub fn is_same_event_pos(&self, event_pos: KeyboardEventPos) -> bool {
        if let KeyboardEventPos::Key(KeyPos { row, col }) = event_pos {
            if self.is_vertical_chord {
                return self.is_same_hand(row as usize);
            } else {
                return self.is_same_hand(col as usize);
            }
        } else {
            return false;
        }
    }

    pub fn is_same_hand(&self, number: usize) -> bool {
        match self.hand {
            ChordHoldHand::Left => number < COUNT / 2,
            ChordHoldHand::Right => number >= COUNT / 2,
        }
    }

    /// Create a new `ChordHoldState` based on the key event and the number of rows and columns.
    /// If the number of columns is greater than the number of rows, it will determine the hand based on the column.
    /// the chordal hold will be determined by user configuration in future.
    pub(crate) fn create(pos: KeyPos, rows: usize, cols: usize) -> Self {
        if cols > rows {
            if (pos.col as usize) < (cols / 2) {
                ChordHoldState {
                    is_vertical_chord: false,
                    hand: ChordHoldHand::Left,
                }
            } else {
                ChordHoldState {
                    is_vertical_chord: false,
                    hand: ChordHoldHand::Right,
                }
            }
        } else if (pos.row as usize) < (rows / 2) {
            ChordHoldState {
                is_vertical_chord: true,
                hand: ChordHoldHand::Left,
            }
        } else {
            ChordHoldState {
                is_vertical_chord: true,
                hand: ChordHoldHand::Right,
            }
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use heapless::Vec;

    use super::{ChordHoldHand, ChordHoldState, KeyPos};
    use crate::event::KeyboardEvent;

    #[test]
    fn test_chordal_hold() {
        assert_eq!(
            ChordHoldState::<6>::create(KeyPos { row: 0, col: 0 }, 3, 6).hand,
            ChordHoldHand::Left
        );
        assert_eq!(
            ChordHoldState::<6>::create(KeyPos { row: 3, col: 3 }, 4, 6).hand,
            ChordHoldHand::Right
        );
        assert_eq!(
            ChordHoldState::<6>::create(KeyPos { row: 3, col: 3 }, 6, 4).hand,
            ChordHoldHand::Right
        );
        assert_eq!(
            ChordHoldState::<6>::create(KeyPos { row: 3, col: 6 }, 5, 3).hand,
            ChordHoldHand::Right
        );

        let chord = ChordHoldState::<6> {
            is_vertical_chord: false,
            hand: ChordHoldHand::Left,
        };

        let vec: Vec<_, 6> = Vec::from_slice(&[0u8, 1, 2, 3, 4, 5]).unwrap();
        let result: Vec<_, 6> = vec
            .iter()
            .map(|col| chord.is_same_hand_key_pos(KeyPos { row: 0, col: *col }))
            .collect();

        let result2: Vec<bool, 6> = Vec::from_slice(&[true, true, true, false, false, false]).unwrap();
        assert_eq!(result, result2);
    }
}
