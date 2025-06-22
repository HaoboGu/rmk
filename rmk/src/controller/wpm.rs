use crate::{
    channel::{send_controller_event, ControllerPub, ControllerSub, CONTROLLER_CHANNEL},
    event::{ControllerEvent, KeyEvent},
};

use super::{Controller, PollingController};

const CHARS_PER_WORD: u8 = 5;
const SAMPLES: u8 = 5;

/// Controller to estimate typing speed in words per minute (WPM)
pub(crate) struct WpmController {
    sub: ControllerSub,
    publisher: ControllerPub,
    keys_pressed: u8,
    wpm: u16,
    update_count: u8,
}

impl WpmController {
    pub fn new() -> Self {
        Self {
            sub: unwrap!(CONTROLLER_CHANNEL.subscriber()),
            publisher: unwrap!(CONTROLLER_CHANNEL.publisher()),
            keys_pressed: 0,
            wpm: 0,
            update_count: 0,
        }
    }
}

impl Controller for WpmController {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        if let ControllerEvent::Key(KeyEvent { pressed: false, .. }, _) = event {
            self.keys_pressed += 1
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}

impl PollingController for WpmController {
    const INTERVAL: embassy_time::Duration = embassy_time::Duration::from_secs(1);

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
            send_controller_event(&mut self.publisher, ControllerEvent::Wpm(self.wpm));
        }

        self.keys_pressed = 0;
    }
}
