use embassy_time::Instant;

use crate::action::KeyAction;
use crate::event::KeyEvent;

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TapHoldDecision {
    // Hold timeout, trigger the hold action
    Timeout,
    // Clean holding buffer
    CleanBuffer,
    // Holding
    Hold,
    // Chordal holding
    ChordHold,
    // Hold on pressing, reserved
    HoldOnPress,
    // Skip key action processing and buffer key event
    Buffering,
    // Continue processing as normal key event
    Ignore,
}

impl TapHoldDecision {
    fn is_hold(&self) -> bool {
        matches!(self, Self::Timeout | Self::Hold | Self::ChordHold | Self::HoldOnPress)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HoldingKey {
    pub state: TapHoldState,
    pub key_event: KeyEvent,
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
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TapDanceState {
    pub tap_count: u8,
    pub last_tap_time: Option<Instant>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TapHoldState {
    /// After a press event is received
    Initial,
    /// Waiting for combo
    WaitingCombo,
    /// Key is marked as Tap, waiting to be processed
    BeforeTap,
    /// Tap key has been processed and sent to HID, but not yet released
    PostTap,
    /// Tap-hold key has been determined as Hold, waiting to be processed
    BeforeHold,
    /// Key is being held, but not yet released
    PostHold,
    /// Key needs to be released but is still in the queue;
    /// should be cleaned up in the main loop regardless
    Release,
    /// Tap-dance state: represents the number of taps completed
    /// Tap(1) = first tap completed, Tap(2) = second tap completed, etc.
    Tap(u8),
    /// Hold after tap(x) state for tap-dance keys
    HoldAfterTap(u8),
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
    pub fn is_same(&self, key_event: KeyEvent) -> bool {
        if self.is_vertical_chord {
            self.is_same_hand(key_event.row as usize)
        } else {
            self.is_same_hand(key_event.col as usize)
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
    pub(crate) fn create(event: KeyEvent, rows: usize, cols: usize) -> Self {
        if cols > rows {
            if (event.col as usize) < (cols / 2) {
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
        } else if (event.row as usize) < (rows / 2) {
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

    use super::{ChordHoldHand, ChordHoldState};
    use crate::event::KeyEvent;

    #[test]
    fn test_chordal_hold() {
        assert_eq!(
            ChordHoldState::<6>::create(
                KeyEvent {
                    row: 0,
                    col: 0,
                    pressed: true,
                },
                3,
                6
            )
            .hand,
            ChordHoldHand::Left
        );
        assert_eq!(
            ChordHoldState::<6>::create(
                KeyEvent {
                    row: 3,
                    col: 3,
                    pressed: true,
                },
                4,
                6
            )
            .hand,
            ChordHoldHand::Right
        );
        assert_eq!(
            ChordHoldState::<6>::create(
                KeyEvent {
                    row: 3,
                    col: 3,
                    pressed: true,
                },
                6,
                4
            )
            .hand,
            ChordHoldHand::Right
        );
        assert_eq!(
            ChordHoldState::<6>::create(
                KeyEvent {
                    row: 6,
                    col: 3,
                    pressed: true,
                },
                5,
                3
            )
            .hand,
            ChordHoldHand::Right
        );

        let chord = ChordHoldState::<6> {
            is_vertical_chord: false,
            hand: ChordHoldHand::Left,
        };

        let vec: Vec<_, 6> = Vec::from_slice(&[0u8, 1, 2, 3, 4, 5]).unwrap();
        let result: Vec<_, 6> = vec
            .iter()
            .map(|col| {
                chord.is_same(KeyEvent {
                    row: 0,
                    col: 0 + col,
                    pressed: true,
                })
            })
            .collect();

        let result2: Vec<bool, 6> = Vec::from_slice(&[true, true, true, false, false, false]).unwrap();
        assert_eq!(result, result2);
    }
}
