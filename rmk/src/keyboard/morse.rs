// TODO: Move morse processing to this module

use embassy_time::{Duration, Instant};

use crate::TAP_DANCE_MAX_TAP;
use crate::action::KeyAction;
use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;
use crate::keyboard::held_buffer::{HeldKey, KeyState};
use crate::morse::Morse;

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    // When a morse key reaches timeout
    // - Trigger the current key as tap or hold according to it's `KeyState`
    // - For all non tap-hold keys pressed, trigger their tap action.
    pub(crate) async fn handle_morse_timeout(&mut self, key: &HeldKey, morse: Morse<TAP_DANCE_MAX_TAP>) {
        match key.state {
            KeyState::Held(tap) => {
                let a = morse.hold_action(tap as usize);
                self.process_key_action_normal(a, key.event).await;
                if let Some(k) = self.held_buffer.find_pos_mut(key.event.pos) {
                    k.state = KeyState::PostHold(tap)
                }
            }
            KeyState::IdleAfterTap(tap) => {
                let a = morse.tap_action(tap as usize);
                self.process_key_action_tap(a, key.event).await;
                let _ = self.held_buffer.remove(key.event.pos);
            }
            _ => unreachable!(),
        };

        // TODO: Now timeout cleans only non-morse keys in the buffer, do we need to clean all cleanable keys in the buffer?
        self.trigger_held_non_morse_keys().await;
    }

    pub(crate) async fn process_key_action_morse(&mut self, morse: Morse<TAP_DANCE_MAX_TAP>, event: KeyboardEvent) {
        debug!("Processing morse: {:?}", event);

        // Process the morse key
        if event.pressed {
            // Pressed, check the held buffer, update the tap state
            let pressed_time = self.get_timer_value(event).unwrap_or(Instant::now());
            match self.held_buffer.find_pos_mut(event.pos) {
                Some(k) => {
                    // The current key is already in the buffer, update the tap state
                    if let KeyState::IdleAfterTap(t) = k.state {
                        let tap_len = morse.tap_actions.len() as u8;
                        if t + 1 >= tap_len {
                            // Reach maximum tapping number
                            k.state = KeyState::Held(tap_len - 1);
                        } else {
                            k.state = KeyState::Held(t + 1);
                        }
                        k.press_time = pressed_time;
                        k.timeout_time = pressed_time + Duration::from_millis(morse.timeout_ms as u64);
                    }
                }
                None => {
                    // Add to buffer
                    self.held_buffer.push(HeldKey::new(
                        event,
                        KeyAction::Morse(morse),
                        KeyState::Held(0),
                        pressed_time,
                        pressed_time + Duration::from_millis(morse.timeout_ms as u64),
                    ));
                }
            }
        } else {
            // Release a morse key
            // 1. It's in the holding buffer
            // 2. If it's already timeout, get the hold action to be released.
            // 3. If it's not timeout, and the releasing action is the last tap actions, and there's no tap actions after it, trigger it immediately
            // 4. Otherwise, update the tap state to idle, wait for the next tap or idle timeout
            if let Some(k) = self.held_buffer.find_pos_mut(event.pos) {
                debug!("Releasing morse key: {:?}", k);
                let action = match k.state {
                    KeyState::Held(t) => {
                        // If the current pressed key is timeout when releasing it, release the hold action
                        if k.timeout_time < Instant::now() {
                            // Timeout, release current hold action
                            Some(morse.hold_action(t as usize))
                        } else {
                            // Not timeout, check whether it's the last tap action
                            if t + 1 == morse.tap_actions.len() as u8 {
                                // It's the last tap action, trigger the tap action immediately
                                let action = morse.tap_action(t as usize);
                                debug!("Last tap action, trigger tap action {:?} immediately", action);
                                // Trigger the tap action immediately
                                k.state = KeyState::PostTap(t);
                                let mut press_event = event;
                                press_event.pressed = true;
                                self.process_key_action_tap(action, press_event).await;
                                self.held_buffer.remove(event.pos);
                                None
                            } else {
                                // It's not the last tap action, update the tap state to idle
                                k.state = KeyState::IdleAfterTap(t);
                                // Use current release time for `IdleAfterTap` state
                                k.press_time = Instant::now(); // Use release time as the "press_time"
                                k.timeout_time = k.press_time + Duration::from_millis(morse.timeout_ms as u64);
                                None
                            }
                        }
                    }
                    KeyState::PostHold(t) => {
                        // Releasing a tap-hold action whose hold action is already triggered
                        info!("Releasing a tap-hold action whose hold action is already triggered");
                        Some(morse.hold_action(t as usize))
                    }
                    KeyState::PostTap(t) => {
                        // Releasing a tap-hold action whose tap action is already triggered
                        info!("Releasing a tap-hold action whose tap action is already triggered");
                        Some(morse.tap_action(t as usize))
                    }
                    _ => {
                        // Release when tap-dance key is in other state, ignore
                        None
                    }
                };

                // If there's an action determined to be triggered, process it
                if let Some(action) = action {
                    debug!("[morse] Releasing tap-hold key: {:?}", event);
                    let _ = self.held_buffer.remove(event.pos);
                    // Process the action
                    self.process_key_action_normal(action, event).await;
                    // Clear timer
                    self.set_timer_value(event, None);
                }
            }
        }
    }

    pub(crate) async fn trigger_held_non_morse_keys(&mut self) {
        self.held_buffer.keys.sort_unstable_by_key(|k| k.press_time);

        // Trigger all non-morse keys in the buffer
        while let Some(key) = self.held_buffer.remove_if(|k| !matches!(k.action, KeyAction::Morse(_))) {
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
