use rmk_macro::controller;

use super::PollingController;
use crate::event::{KeyEvent, ModifierEvent};
use crate::event::{KeyboardEvent, publish_controller_event};

const CHARS_PER_WORD: u8 = 5;
const SAMPLES: u8 = 5;

/// Controller to estimate typing speed in words per minute (WPM)
#[controller(subscribe = [KeyEvent, ModifierEvent])]
pub(crate) struct WpmController {
    keys_pressed: u8,
    wpm: u16,
    update_count: u8,
}

impl WpmController {
    pub fn new() -> Self {
        Self {
            keys_pressed: 0,
            wpm: 0,
            update_count: 0,
        }
    }

    async fn on_key_event(&mut self, event: KeyEvent) {
        if let KeyboardEvent { pressed: false, .. } = event.keyboard_event {
            self.keys_pressed += 1
        }
    }

    async fn on_modifier_event(&mut self, _event: ModifierEvent) {
        // No action needed for modifier events
    }
}

impl PollingController for WpmController {
    fn interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_secs(1)
    }

    async fn update(&mut self) {
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
            publish_controller_event(crate::event::WpmUpdateEvent { wpm: self.wpm });
        }

        self.keys_pressed = 0;
    }
}
