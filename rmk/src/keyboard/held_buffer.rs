use embassy_time::Instant;

use crate::action::KeyAction;
use crate::event::{KeyboardEvent, KeyboardEventPos};

/// The buffer of held keys.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HeldBuffer {
    // TODO: Make the buffer size configurable
    pub(crate) keys: heapless::Vec<HeldKey, 16>,
}

impl HeldBuffer {
    /// Create a new held buffer
    pub fn new() -> Self {
        Self {
            keys: heapless::Vec::new(),
        }
    }

    /// Push a new held key into the buffer and then sort by the timeout
    pub fn push(&mut self, key: HeldKey) {
        if let Err(e) = self.keys.push(key) {
            error!("Held buffer overflowed, cannot save: {:?}", e);
        }

        // Sort the buffer after push
        self.keys.sort_unstable_by_key(|k| k.timeout_time);
    }

    /// Push a new held key into the buffer
    pub fn push_without_sort(&mut self, key: HeldKey) {
        if let Err(e) = self.keys.push(key) {
            error!("Held buffer overflowed, cannot save: {:?}", e);
        }
    }

    /// Find a held key by the key action
    pub fn find_action(&self, action: KeyAction) -> Option<&HeldKey> {
        self.keys.iter().find(|x| x.action == action)
    }

    /// Find a held key by the KeyboardEventPos
    pub fn find_pos(&self, pos: KeyboardEventPos) -> Option<&HeldKey> {
        self.keys.iter().find(|x| x.event.pos == pos)
    }

    /// Find a mutable held key by the KeyboardEventPos
    pub fn find_pos_mut(&mut self, pos: KeyboardEventPos) -> Option<&mut HeldKey> {
        self.keys.iter_mut().find(|x| x.event.pos == pos)
    }

    /// Remove a held key from the buffer, keep the order
    pub fn remove_if<P>(&mut self, mut predicate: P) -> Option<HeldKey>
    where
        P: FnMut(&HeldKey) -> bool,
    {
        if let Some(i) = self.keys.iter().position(|x| predicate(x)) {
            Some(self.keys.remove(i))
        } else {
            None
        }
    }

    /// Remove a held key from the buffer and then resort the buffer
    pub fn remove(&mut self, pos: KeyboardEventPos) -> Option<HeldKey> {
        let k = self.remove_if(|k| k.event.pos == pos);
        // Re-sort the buffer after remove
        self.keys.sort_unstable_by_key(|k| k.timeout_time);
        k
    }

    /// Get the next timeout key in the buffer
    pub fn next_timeout<P>(&self, mut predicate: P) -> Option<HeldKey>
    where
        P: FnMut(&HeldKey) -> bool,
    {
        // Support that the held buffer is already sorted by the timeout time
        self.keys.iter().filter(|&x| predicate(x)).next().map(|x| x.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// The state of a held key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyState {
    /// After a press event is received.
    /// The number represents the number of completed "tap".
    Held(u8),
    /// Idle state after tap n times for morse keys
    IdleAfterTap(u8),
    /// Idle state after hold released
    IdleAfterHold(u8),
    /// The current key is a component of a combo, and it's waiting for other combo components
    WaitingCombo,
    /// Tap key has been processed and sent to HID, but not yet released.
    /// The number is used for morse keys, represents the number of completed "tap"s
    PostTap(u8),
    /// Key is being held, but not yet released
    /// The number is used for morse keys, represents the number of completed "tap"s
    PostHold(u8),
    /// Key needs to be released but is still in the queue,
    /// it should be cleaned up in the main loop regardless
    Release,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HeldKey {
    pub event: KeyboardEvent,
    pub action: KeyAction,
    /// Current state of the held key
    pub state: KeyState,
    /// The press time for the key
    pub press_time: Instant,
    /// The timeout time for the key
    pub timeout_time: Instant,
}

impl HeldKey {
    pub fn new(
        event: KeyboardEvent,
        action: KeyAction,
        state: KeyState,
        press_time: Instant,
        timeout_time: Instant,
    ) -> Self {
        Self {
            event,
            action,
            state,
            press_time,
            timeout_time,
        }
    }
}
