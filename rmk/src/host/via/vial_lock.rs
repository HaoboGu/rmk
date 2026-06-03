use core::cell::Cell;

use crate::keymap::KeyMap;

/// Vial unlock gate. State (`unlocked`, `unlocking`, `last_poll`) lives behind
/// `Cell`s so the lock can be shared by reference across concurrent USB and
/// BLE Vial sessions without an async mutex. Safe because every mutation is a
/// non-`await` `Cell::set` on `Copy` data, so no borrow ever crosses an
/// `.await` point.
pub(crate) struct VialLock<'a> {
    unlocked: Cell<bool>,
    unlocking: Cell<bool>,
    last_poll: Cell<embassy_time::Instant>,
    unlock_keys: &'a [(u8, u8)],
    keymap: &'a KeyMap<'a>,
}

impl<'a> VialLock<'a> {
    pub fn new(unlock_keys: &'a [(u8, u8)], keymap: &'a KeyMap<'a>) -> Self {
        Self {
            unlocked: Cell::new(false),
            unlocking: Cell::new(false),
            last_poll: Cell::new(embassy_time::Instant::MIN),
            unlock_keys,
            keymap,
        }
    }
    pub fn is_unlocking(&self) -> bool {
        self.update_unlocking_state();
        self.unlocking.get()
    }
    pub fn is_unlocked(&self) -> bool {
        self.unlocked.get()
    }
    pub fn unlocking(&self) {
        self.unlocking.set(true);
        self.last_poll.set(embassy_time::Instant::now());
    }
    pub fn unlock(&self) {
        if self.unlocking.get() {
            self.unlocked.set(true);
            self.unlocking.set(false);
        }
    }
    pub fn check_unlock(&self) -> u8 {
        if self.unlock_keys.is_empty() {
            warn!("No unlock keys provided");
            1
        } else {
            let mut counter = self.unlock_keys.len().try_into().unwrap();
            for (row, col) in self.unlock_keys {
                if self.keymap.read_matrix_key(*row, *col) {
                    counter -= 1;
                }
            }
            if counter == 0 {
                self.unlock();
            }
            counter
        }
    }
    pub fn lock(&self) {
        self.unlocked.set(false);
    }
    fn update_unlocking_state(&self) {
        if self.last_poll.get().elapsed() > embassy_time::Duration::from_millis(100) {
            self.unlocking.set(false);
        }
    }
}
