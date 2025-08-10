use embassy_time::{Duration, Instant};

use crate::MAX_MORSE_PATTERNS_PER_KEY;
use crate::action::KeyAction;
use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;
use crate::keyboard::held_buffer::{HeldKey, KeyState};
use crate::morse::{Morse, MorsePattern};

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    // When a morse key reaches timeout
    // Trigger the current pattern according to it's `KeyState`
    pub(crate) async fn handle_morse_timeout(&mut self, key: &HeldKey, morse: &Morse<MAX_MORSE_PATTERNS_PER_KEY>) {
        match key.state {
            KeyState::Held(pattern) => {
                let a = morse.action_from_pattern(pattern.followed_by_hold());
                self.process_key_action_normal(a, key.event).await; //FIXME: in real morse patterns the hold can be followed by taps, so do not fire immediately!
                if let Some(k) = self.held_buffer.find_pos_mut(key.event.pos) {
                    k.state = KeyState::PostHold(pattern.followed_by_hold())
                }
            }
            KeyState::IdleAfterTap(pattern) => {
                let a = morse.action_from_pattern(pattern); //FIXME: is this correct? is followed_by_hold/tap() needed here? //long idle, means the end of the morse pattern
                self.process_key_action_tap(a, key.event).await;
                let _ = self.held_buffer.remove(key.event.pos);
            }
            _ => unreachable!(),
        };

        // If there's still morse key in the held buffer, don't fire normal keys
        if self
            .held_buffer
            .keys
            .iter()
            .any(|k| k.morse.is_some() && matches!(k.state, KeyState::Held(_)))
        //FIXME: is this correct? or KeyState::Held(MorsePattern::default()) needed here?
        {
            return;
        }

        self.fire_held_non_morse_keys().await;
    }

    pub(crate) async fn process_key_action_morse(
        &mut self,
        key_action: KeyAction,
        morse: &Morse<MAX_MORSE_PATTERNS_PER_KEY>,
        event: KeyboardEvent,
    ) {
        debug!("Processing morse: {:?}", event);

        // Process the morse key
        if event.pressed {
            // Pressed, check the held buffer, update the tap state
            let pressed_time = self.get_timer_value(event).unwrap_or(Instant::now());
            match self.held_buffer.find_pos_mut(event.pos) {
                Some(k) => {
                    // The current key is already in the buffer, update the tap state
                    if let KeyState::IdleAfterTap(pattern) = k.state {
                        if pattern.is_full() || pattern.pattern_length() >= morse.max_pattern_length() {
                            //FIXME: is this correct?
                            // Reach maximum tapping number
                            k.state = KeyState::Held(pattern);
                        } else {
                            k.state = KeyState::Held(pattern.followed_by_tap()); //FIXME: is this correct? is followed_by_tap/hold() needed here?
                        }
                        k.press_time = pressed_time;
                        k.timeout_time = pressed_time + Duration::from_millis(morse.timeout_ms as u64);
                    }
                }
                None => {
                    // Add to buffer
                    self.held_buffer.push(HeldKey::new(
                        event,
                        key_action,
                        Some(morse.clone()),
                        KeyState::Held(MorsePattern::default()), //FIXME: is this correct? is followed_by_hold/tap() needed here?
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
                    KeyState::Held(pattern) => {
                        // If the current pressed key is timeout when releasing it, release the hold action
                        if k.timeout_time < Instant::now() {
                            // Timeout, release current hold action
                            Some(morse.action_from_pattern(pattern.followed_by_hold())) //FIXME: is this correct? is followed_by_hold/tap() needed here?
                        } else {
                            // Not timeout, check whether it's the last tap action
                            if pattern.is_full() || pattern.pattern_length() >= morse.max_pattern_length() {
                                //FIXME: is this correct?
                                // It's the last tap action, trigger the tap action immediately
                                let action = morse.action_from_pattern(pattern); //FIXME: is this correct? is followed_by_hold/tap() needed here?
                                debug!("Last tap action, trigger tap action {:?} immediately", action);
                                // Trigger the tap action immediately
                                k.state = KeyState::PostTap(pattern);
                                let mut press_event = event;
                                press_event.pressed = true;
                                self.process_key_action_tap(action, press_event).await;
                                self.held_buffer.remove(event.pos);
                                None
                            } else {
                                // It's not the last tap action, update the tap state to idle
                                k.state = KeyState::IdleAfterTap(pattern.followed_by_tap()); //FIXME: is this correct? is followed_by_tap() needed here?
                                // Use current release time for `IdleAfterTap` state
                                k.press_time = Instant::now(); // Use release time as the "press_time"
                                k.timeout_time = k.press_time + Duration::from_millis(morse.timeout_ms as u64);
                                None
                            }
                        }
                    }
                    KeyState::PostHold(pattern) => {
                        // Releasing a tap-hold action whose hold action is already triggered
                        info!("Releasing a tap-hold action whose hold action is already triggered");
                        Some(morse.action_from_pattern(pattern)) //FIXME: is this correct? is followed_by_hold/tap() needed here?
                    }
                    KeyState::PostTap(pattern) => {
                        // Releasing a tap-hold action whose tap action is already triggered
                        info!("Releasing a tap-hold action whose tap action is already triggered");
                        Some(morse.action_from_pattern(pattern)) //FIXME: is this correct? is followed_by_hold/tap() needed here?
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

    pub(crate) async fn fire_held_non_morse_keys(&mut self) {
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
