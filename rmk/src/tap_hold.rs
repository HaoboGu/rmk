use crate::event::{KeyPos, KeyboardEventPos};

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TapHoldDecision {
    // Clean holding buffer due to permissive hold is triggered
    CleanBuffer,
    // Skip key action processing and buffer key event
    Buffer,
    // Continue processing as normal key event
    Ignore,
    // Release current key
    Release,
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HeldKeyDecision {
    // Ignore it
    Ignore,
    // Chordal hold triggered
    ChordalHold,
    // Permissive hold triggered
    PermissiveHold,
    // Hold on other key press triggered
    HoldOnOtherKeyPress,
    // Used for the buffered key which is releasing now
    Release,
    // Releasing a key that is pressed before any keys in the buffer
    NotInBuffer,
    // The held key is a normal key,
    // It will always be added to the decision list, and the decision will be made later
    Normal,
}

// #[derive(Clone, Debug)]
// #[cfg_attr(feature = "defmt", derive(defmt::Format))]
// pub struct HoldingKey {
//     pub state: TapHoldState,
//     pub event: KeyboardEvent,
//     // TODO: remove it, using `Keyboard.timer` instead
//     pub pressed_time: Instant,
//     pub action: KeyAction,
// }

// impl HoldingKey {
//     pub(crate) fn is_tap_hold(&self) -> bool {
//         matches!(self.action, KeyAction::TapHold(_, _))
//     }

//     pub(crate) fn is_tap_dance(&self) -> bool {
//         matches!(self.action, KeyAction::TapDance(_))
//     }

//     pub(crate) fn update_state(&mut self, new_state: TapHoldState) {
//         self.state = new_state;
//     }

//     pub(crate) fn press_time(&self) -> Instant {
//         self.pressed_time
//     }

//     pub(crate) fn state(&self) -> TapHoldState {
//         self.state
//     }

//     pub(crate) fn tap_num(&self) -> u8 {
//         match self.state {
//             TapHoldState::Tap(num) => num,
//             _ => 0,
//         }
//     }
// }

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
pub struct ChordHoldState {
    pub is_vertical_chord: bool,
    pub hand: ChordHoldHand,
    pub count: u8,
}

impl ChordHoldState {
    // is the key event in the same side of current chord hold
    pub fn is_same_hand(&self, key: KeyboardEventPos) -> bool {
        match key {
            KeyboardEventPos::Key(key_pos) => {
                if self.is_vertical_chord {
                    self.is_same_hand_inner(key_pos.row as usize)
                } else {
                    self.is_same_hand_inner(key_pos.col as usize)
                }
            }
            KeyboardEventPos::RotaryEncoder(_) => false,
        }
    }

    pub fn is_same_hand_inner(&self, n: usize) -> bool {
        match self.hand {
            ChordHoldHand::Left => n < self.count as usize / 2,
            ChordHoldHand::Right => n >= self.count as usize / 2,
        }
    }

    /// Create a new `ChordHoldState` based on the key event and the number of rows and columns.
    /// If the number of columns is greater than the number of rows, it will determine the hand based on the column.
    /// the chordal hold will be determined by user configuration in future.
    pub(crate) fn create(pos: KeyPos, rows: u8, cols: u8) -> Self {
        if cols > rows {
            if pos.col < (cols / 2) {
                ChordHoldState {
                    is_vertical_chord: false,
                    hand: ChordHoldHand::Left,
                    count: cols,
                }
            } else {
                ChordHoldState {
                    is_vertical_chord: false,
                    hand: ChordHoldHand::Right,
                    count: cols,
                }
            }
        } else if pos.row < (rows / 2) {
            ChordHoldState {
                is_vertical_chord: true,
                hand: ChordHoldHand::Left,
                count: rows,
            }
        } else {
            ChordHoldState {
                is_vertical_chord: true,
                hand: ChordHoldHand::Right,
                count: rows,
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
            ChordHoldState::create(KeyPos { row: 0, col: 0 }, 3, 6).hand,
            ChordHoldHand::Left
        );
        assert_eq!(
            ChordHoldState::create(KeyPos { row: 3, col: 3 }, 4, 6).hand,
            ChordHoldHand::Right
        );
        assert_eq!(
            ChordHoldState::create(KeyPos { row: 3, col: 3 }, 6, 4).hand,
            ChordHoldHand::Right
        );
        assert_eq!(
            ChordHoldState::create(KeyPos { row: 3, col: 6 }, 6, 3).hand,
            ChordHoldHand::Right
        );

        let chord = ChordHoldState {
            is_vertical_chord: false,
            hand: ChordHoldHand::Left,
            count: 6,
        };

        let vec: Vec<_, 6> = Vec::from_slice(&[0u8, 1, 2, 3, 4, 5]).unwrap();
        let result: Vec<_, 6> = vec
            .iter()
            .map(|col| chord.is_same_hand(crate::event::KeyboardEventPos::Key(KeyPos { row: 0, col: *col })))
            .collect();

        let result2: Vec<bool, 6> = Vec::from_slice(&[true, true, true, false, false, false]).unwrap();
        assert_eq!(result, result2);
    }
}
