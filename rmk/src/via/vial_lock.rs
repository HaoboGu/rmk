use core::cell::RefCell;

use crate::keymap::KeyMap;

pub(crate) struct VialLock<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    unlocked: bool,
    unlocking: bool,
    last_poll: embassy_time::Instant,
    unlock_keys: &'a [(u8, u8)],
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    VialLock<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(unlock_keys: &'a [(u8, u8)], keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
        Self {
            unlocked: false,
            unlocking: false,
            last_poll: embassy_time::Instant::MIN,
            unlock_keys,
            keymap,
        }
    }
    pub fn is_unlocking(&mut self) -> bool {
        self.update_unlocking_state();
        self.unlocking
    }
    pub fn is_unlocked(&self) -> bool {
        self.unlocked
    }
    pub fn unlocking(&mut self) {
        self.unlocking = true;
        self.last_poll = embassy_time::Instant::now();
    }
    pub fn unlock(&mut self) {
        if self.unlocking {
            self.unlocked = true;
            self.unlocking = false;
        }
    }
    pub fn check_unlock(&mut self) -> u8 {
        if self.unlock_keys.len() == 0 {
            warn!("No unlock keys provided");
            1
        } else {
            let mut counter = self.unlock_keys.len().try_into().unwrap();
            for (row, col) in self.unlock_keys {
                if self.keymap.borrow().matrix_state.read(*row, *col) {
                    counter -= 1;
                }
            }
            if counter == 0 {
                self.unlock();
            }
            counter
        }
    }
    pub fn lock(&mut self) {
        self.unlocked = false;
    }
    fn update_unlocking_state(&mut self) {
        if self.last_poll.elapsed() > embassy_time::Duration::from_millis(100) {
            self.unlocking = false;
        }
    }
}
