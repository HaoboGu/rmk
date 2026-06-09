//! WPM (Words Per Minute) processor for RMK
use core::sync::atomic::{AtomicU16, Ordering};

use rmk_macro::processor;

use crate::event::{KeyboardEvent, WpmUpdateEvent, publish_event};

const CHARS_PER_WORD: u8 = 5;
const SAMPLES: u8 = 5;

/// Latest WPM, written alongside every `WpmUpdateEvent` publish so host
/// services can read the current value synchronously without subscribing.
static CURRENT_WPM: AtomicU16 = AtomicU16::new(0);

/// Current typing speed in words per minute. Returns `0` when no
/// `WpmProcessor` is running.
pub(crate) fn current_wpm() -> u16 {
    CURRENT_WPM.load(Ordering::Relaxed)
}

/// Processor to estimate typing speed in words per minute (WPM)
#[processor(subscribe = [KeyboardEvent], poll_interval = 1000)]
pub struct WpmProcessor {
    keys_pressed: u8,
    wpm: u16,
    update_count: u8,
}

impl Default for WpmProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl WpmProcessor {
    pub fn new() -> Self {
        Self {
            keys_pressed: 0,
            wpm: 0,
            update_count: 0,
        }
    }

    async fn on_keyboard_event(&mut self, event: KeyboardEvent) {
        if let KeyboardEvent { pressed: false, .. } = event {
            self.keys_pressed += 1
        }
    }

    async fn poll(&mut self) {
        self.update_count = SAMPLES.min(self.update_count + 1);

        let instant_wpm = self.keys_pressed as u16 * 60 / CHARS_PER_WORD as u16;

        let avg_wpm = if instant_wpm > 0 {
            (self.wpm * (self.update_count - 1) as u16 + instant_wpm) / self.update_count as u16
        } else {
            self.update_count = 0;
            0
        };

        if avg_wpm != self.wpm {
            self.wpm = avg_wpm;
            CURRENT_WPM.store(self.wpm, Ordering::Relaxed);
            publish_event(WpmUpdateEvent::new(self.wpm));
        }

        self.keys_pressed = 0;
    }
}
