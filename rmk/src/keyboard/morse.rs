use embassy_time::{Duration, Instant};
use rmk_types::action::{Action, KeyAction};

use crate::config::BehaviorConfig;
use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;
use crate::keyboard::held_buffer::{HeldKey, KeyState};
use crate::morse::{HOLD, MorseMode, MorsePattern, TAP};

// 'morse' is an alias for the superset of tap dance and tap hold keys, since their handling have many similarities
impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    // When a morse key reaches timeout after press / release
    pub(crate) async fn handle_morse_timeout(&mut self, key: &HeldKey) {
        assert!(key.action.is_morse());

        match key.state {
            KeyState::Pressed(pattern) => {
                // The time since the key press is longer than the timeout,
                // if there is no possibility for longer morse patterns, trigger the action:
                let pattern = pattern.followed_by_hold();
                debug!("pattern while holding: {:?}", pattern);
                let final_action = Self::try_predict_final_action(&self.keymap.borrow().behavior, &key.action, pattern);
                if let Some(action) = final_action {
                    debug!("hold prediction {:?} -> {:?}", pattern, action);
                    self.process_key_action_normal(action, key.event).await;
                    if let Some(k) = self.held_buffer.find_pos_mut(key.event.pos) {
                        k.state = KeyState::ProcessedButReleaseNotReportedYet(action);
                    }
                } else {
                    // Expect a possible longer morse pattern (or idle timeout after release), so can not finish yet...
                    // Update the state so this test will not run again until the next keypress.
                    if let Some(k) = self.held_buffer.find_pos_mut(key.event.pos) {
                        k.state = KeyState::Holding(pattern);
                    }
                }
            }
            KeyState::Released(pattern) => {
                // The time since the key release is longer than the timeout, trigger the action
                let action = Self::action_from_pattern(&self.keymap.borrow().behavior, &key.action, pattern);
                self.process_key_action_tap(action, key.event).await;
                let _ = self.held_buffer.remove(key.event.pos); // Removing from the held buffer is like setting to an idle state
            }
            _ => unreachable!(),
        };

        // If there's still morse key in the held buffer, don't fire normal keys
        // FIXME? is |Holding needed here?
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

    pub(crate) async fn process_key_action_morse(&mut self, key_action: &KeyAction, event: KeyboardEvent) {
        debug!("Processing morse keys: {:?}", event);
        assert!(key_action.is_morse());

        // Process the morse key
        if event.pressed {
            // Pressed, check the held buffer, update the tap state
            let pressed_time = self.get_timer_value(event).unwrap_or(Instant::now());
            let timeout_time = pressed_time + Self::morse_timeout(&self.keymap.borrow().behavior, key_action);
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
                        *key_action,
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
                        let released_time = Instant::now(); // TODO? It would be better if the event would carry the real timestamp of the release event!

                        let hold = released_time >= k.timeout_time;

                        let pattern = if hold {
                            debug!("pattern after hold release: {:?}", pattern);
                            pattern.followed_by_hold()
                        } else {
                            debug!("pattern after tap release: {:?}", pattern);
                            pattern.followed_by_tap()
                        };

                        let final_action =
                            Self::try_predict_final_action(&self.keymap.borrow().behavior, &k.action, pattern);
                        if let Some(action) = final_action {
                            debug!("released prediction {:?} -> {:?}", pattern, action);
                            // Reached the longest configured morse pattern, trigger the corresponding action immediately
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
                            k.press_time = released_time; // Use release time as the "press_time"
                            k.timeout_time =
                                k.press_time + Self::morse_timeout(&self.keymap.borrow().behavior, &k.action);
                        }
                    }
                    KeyState::Holding(pattern) => {
                        // The try_predict_final_action => None is already decided, when we entered in Holding mode
                        // So, just expect a possible longer morse pattern (or idle timeout), update the state
                        let released_time = Instant::now(); // TODO? It would be better if the event would carry the real timestamp of the release event!                        
                        k.state = KeyState::Released(pattern);
                        // Use current release time for `IdleAfterTap` state
                        k.press_time = released_time; // Use release time as the "press_time"
                        k.timeout_time = k.press_time + Self::morse_timeout(&self.keymap.borrow().behavior, &k.action);
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

        // Trigger all non morse keys in the buffer
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

    pub fn action_from_pattern(
        behavior_config: &BehaviorConfig,
        keyAction: &KeyAction,
        pattern: MorsePattern,
    ) -> Action {
        match keyAction {
            KeyAction::TapHold(tap_action, hold_action) => match pattern {
                TAP => *tap_action,
                HOLD => *hold_action,
                _ => Action::No,
            },
            KeyAction::Morse(idx) => behavior_config
                .morse
                .morses
                .get(*idx as usize)
                .map(|morse| morse.get(pattern).unwrap_or(Action::No))
                .unwrap_or(Action::No),
            _ => Action::No,
        }
    }

    pub fn morse_timeout(behavior_config: &BehaviorConfig, keyAction: &KeyAction) -> Duration {
        match keyAction {
            KeyAction::Morse(idx) => behavior_config
                .morse
                .morses
                .get(*idx as usize)
                .map(|td| Duration::from_millis(td.timeout_ms as u64)),
            _ => None,
        }
        .unwrap_or_else(|| behavior_config.tap_hold.timeout)
    }

    /// Decides and returns the pair of (tap_hold_mode, unilateral_tap) based on configuration for the given key action
    pub fn tap_hold_mode(behavior_config: &BehaviorConfig, key_action: &KeyAction) -> (MorseMode, bool) {
        match key_action {
            KeyAction::Morse(idx) => behavior_config
                .morse
                .morses
                .get(*idx as usize)
                .map(|td| (td.mode, td.unilateral_tap)),
            _ => None,
        }
        .unwrap_or_else(|| {
            if behavior_config.tap_hold.enable_hrm // TODO instead of this let the HRM keycodes configurable!
                && let Action::Key(tap_key_code) = Self::action_from_pattern(behavior_config, key_action, TAP)
            {
                if tap_key_code.is_letter() || tap_key_code.is_home_row() {
                    (MorseMode::PermissiveHold, true)
                } else {
                    match Self::action_from_pattern(behavior_config, key_action, HOLD) {
                        Action::Modifier(_) | Action::LayerOn(_) => {
                            // MT/LT on non-letter, non-home-row keys
                            (MorseMode::HoldOnOtherPress, false)
                        }
                        _ => (behavior_config.tap_hold.mode, behavior_config.tap_hold.unilateral_tap),
                    }
                }
            } else {
                (behavior_config.tap_hold.mode, behavior_config.tap_hold.unilateral_tap)
            }
        })
    }

    //returns Some(action) if the ending of the given pattern can be "predicted" (unique)
    pub fn try_predict_final_action(
        behavior_config: &BehaviorConfig,
        keyAction: &KeyAction,
        pattern_start: MorsePattern,
    ) -> Option<Action> {
        match keyAction {
            KeyAction::TapHold(tap_action, hold_action) => {
                if pattern_start.last_is_hold() {
                    Some(*hold_action)
                } else {
                    Some(*tap_action)
                }
            }
            KeyAction::Morse(idx) => behavior_config
                .morse
                .morses
                .get(*idx as usize)
                .and_then(|td| td.try_predict_final_action(pattern_start)),
            _ => None,
        }
    }
}
