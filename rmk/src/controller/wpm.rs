use super::PollingController;
use crate::builtin_events::KeyboardInputEvent;
use crate::event::{KeyboardEvent, publish_controller_event};
use rmk_macro::controller;

const CHARS_PER_WORD: u8 = 5;
const SAMPLES: u8 = 5;

/// Controller to estimate typing speed in words per minute (WPM)
#[controller(subscribe = [KeyboardInputEvent])]
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

    async fn on_keyboard_input_event(&mut self, event: KeyboardInputEvent) {
        if let KeyboardInputEvent::Key { keyboard_event: KeyboardEvent { pressed: false, .. }, .. } = event {
            self.keys_pressed += 1
        }
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
            publish_controller_event(crate::builtin_events::KeyboardStateEvent::wpm(self.wpm));
        }

        self.keys_pressed = 0;
    }
}
