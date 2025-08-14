use embassy_time::Instant;

use crate::action::KeyAction;
use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;
use crate::keyboard::held_buffer::{HeldKey, KeyState};
use crate::morse::MorsePattern;

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    // When a morse key reaches timeout after press / release
    pub(crate) async fn handle_morse_timeout(&mut self, key: &HeldKey) {
        // !assert(key.action.is_morse());

        match key.state {
            KeyState::Pressed(pattern) => {
                // The time since the key press is longer than the timeout,
                // if there is no possibility for longer morse patterns, trigger the action:
                let pattern = pattern.followed_by_hold();
                if pattern.is_full()
                    || pattern.pattern_length() >= key.action.max_pattern_length(&self.keymap.borrow().behavior)
                {
                    let action = key.action.action_from_pattern(&self.keymap.borrow().behavior, pattern);
                    self.process_key_action_normal(action, key.event).await;
                    if let Some(k) = self.held_buffer.find_pos_mut(key.event.pos) {
                        k.state = KeyState::ProcessedButReleaseNotReportedYet(action);
                    }
                }
            }
            KeyState::Released(pattern) => {
                // The time since the key release is longer than the timeout, trigger the action
                let action = key.action.action_from_pattern(&self.keymap.borrow().behavior, pattern);
                self.process_key_action_tap(action, key.event).await;
                let _ = self.held_buffer.remove(key.event.pos); // Removing from the held buffer is like setting to an idle state
            }
            _ => unreachable!(),
        };

        // If there's still morse key in the held buffer, don't fire normal keys
        if self
            .held_buffer
            .keys
            .iter()
            .any(|k| k.action.is_morse() && matches!(k.state, KeyState::Pressed(_)))
        {
            return; //?
        }

        self.fire_held_non_morse_keys().await;
    }

    pub(crate) async fn process_key_action_morse(&mut self, key_action: KeyAction, event: KeyboardEvent) {
        debug!("Processing morse: {:?}", event);
        // !assert(key_action.is_morse());

        // Process the morse key
        if event.pressed {
            // Pressed, check the held buffer, update the tap state
            let pressed_time = self.get_timer_value(event).unwrap_or(Instant::now());
            let timeout_time = pressed_time + key_action.morse_timeout(&self.keymap.borrow().behavior);
            match self.held_buffer.find_pos_mut(event.pos) {
                Some(k) => {
                    // The current key is already in the buffer, update its state
                    if let KeyState::Released(pattern) = k.state {
                        k.state = KeyState::Pressed(pattern);
                        k.press_time = pressed_time;
                        k.timeout_time = timeout_time;
                    }
                }
                None => {
                    // Add to buffer
                    self.held_buffer.push(HeldKey::new(
                        event,
                        key_action,
                        KeyState::Pressed(MorsePattern::default()),
                        pressed_time,
                        timeout_time,
                    ));
                }
            }
        } else {
            // Release a morse key, which is in the held buffer
            // If there's no possible longer morse pattern, trigger it immediately
            // Otherwise, update the state, wait for the either the next press event or the idle timeout
            if let Some(k) = self.held_buffer.find_pos_mut(event.pos) {
                debug!("Releasing morse key: {:?}", k);
                match k.state {
                    KeyState::Pressed(pattern) => {
                        let pattern = if Instant::now() >= k.timeout_time {
                            pattern.followed_by_hold()
                        } else {
                            pattern.followed_by_tap()
                        };

                        if pattern.is_full()
                            || pattern.pattern_length() >= k.action.max_pattern_length(&self.keymap.borrow().behavior)
                        {
                            // Reached the longest configured morse pattern, trigger the corresponding action immediately
                            let action = k.action.action_from_pattern(&self.keymap.borrow().behavior, pattern);

                            self.held_buffer.remove(event.pos); // Remove the key from the held buffer, is like setting to an idle state

                            debug!(
                                "Reached the longest configured morse pattern, trigger corresponding action {:?} immediately",
                                action
                            );

                            // Trigger the morse action immediately
                            let mut press_event = event;
                            press_event.pressed = true;
                            self.process_key_action_tap(action, press_event).await;
                            self.held_buffer.remove(event.pos); // Remove the key from the held buffer, is like setting to an idle state
                        } else {
                            // Expect a possible longer morse pattern (or idle timeout), update the state
                            k.state = KeyState::Released(pattern);
                            // Use current release time for `IdleAfterTap` state
                            k.press_time = Instant::now(); // Use release time as the "press_time"
                            k.timeout_time = k.press_time + k.action.morse_timeout(&self.keymap.borrow().behavior);
                        }
                    }
                    KeyState::ProcessedButReleaseNotReportedYet(action) => {
                        // Releasing a tap-hold action whose pressed HID report is already sent
                        info!("Releasing a morse action whose pressed action is already triggered");
                        let _ = self.held_buffer.remove(event.pos);
                        // Process the release action
                        debug!("[morse] Releasing morse key: {:?}", event);
                        self.process_key_action_normal(action, event).await;
                        // Clear timer
                        self.set_timer_value(event, None);
                    }
                    _ => {}
                };
            }
        }
    }

    pub(crate) async fn fire_held_non_morse_keys(&mut self) {
        self.held_buffer.keys.sort_unstable_by_key(|k| k.press_time);

        // Trigger all non-morse keys in the buffer
        while let Some(key) = self.held_buffer.remove_if(|k| !k.action.is_morse()) {
            debug!("Trigger non-morse key: {:?}", key);
            let action = self.keymap.borrow_mut().get_action_with_layer_cache(key.event);
            match action {
                KeyAction::Single(action) => self.process_key_action_normal(action, key.event).await,
                KeyAction::Tap(action) => self.process_key_action_tap(action, key.event).await,
                _ => (),
            }
        }

        self.held_buffer.keys.sort_unstable_by_key(|k| k.timeout_time);
    }
}
