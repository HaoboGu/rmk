use embassy_futures::select::{Either, select};
use embassy_time::Timer;
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;

/// State machine for one shot keys
#[derive(Default)]
pub enum OneShotState<T> {
    /// First one shot key press
    Initial(T),
    /// One shot key was released before any other key, normal one shot behavior
    Single(T),
    /// Another key was pressed before one shot key was released, treat as a normal modifier/layer
    Held(T),
    /// One shot inactive
    #[default]
    None,
}

impl<T> OneShotState<T> {
    /// Get the current one shot value if any
    pub fn value(&self) -> Option<&T> {
        match self {
            OneShotState::Initial(v) | OneShotState::Single(v) | OneShotState::Held(v) => Some(v),
            OneShotState::None => None,
        }
    }
}

impl<'a> Keyboard<'a> {
    pub(crate) async fn process_action_osm(&mut self, new_modifiers: ModifierCombination, event: KeyboardEvent) {
        let osm_config = self.keymap.one_shot_modifiers_config();
        let activate_on_keypress = osm_config.activate_on_keypress;

        // Update one shot state
        if event.pressed {
            // Check for re-press of same OSM key (modifier bits overlap with active one-shot)
            if let Some(&active_mods) = self.osm_state.value() {
                let is_repress = active_mods & new_modifiers == new_modifiers;
                if is_repress {
                    if osm_config.retap_cancel {
                        // Cancel one-shot silently
                        self.unprocessed_events.retain(|e| e.pos != event.pos);
                        self.osm_state = OneShotState::None;
                        if activate_on_keypress {
                            self.send_keyboard_report_with_resolved_modifiers(false).await;
                        }
                        return;
                    } else if osm_config.tap_on_double_press {
                        // Send bare modifier tap and consume one-shot
                        self.unprocessed_events.retain(|e| e.pos != event.pos);
                        self.send_keyboard_report_with_resolved_modifiers(true).await;
                        self.osm_state = OneShotState::None;
                        self.send_keyboard_report_with_resolved_modifiers(false).await;
                        return;
                    }
                }
            }

            // Add new modifier combination to existing one shot or init if none
            self.osm_state = match self.osm_state {
                OneShotState::None => OneShotState::Initial(new_modifiers),
                OneShotState::Initial(cur_modifiers) => OneShotState::Initial(cur_modifiers | new_modifiers),
                OneShotState::Single(cur_modifiers) => OneShotState::Single(cur_modifiers | new_modifiers),
                OneShotState::Held(cur_modifiers) => OneShotState::Held(cur_modifiers | new_modifiers),
            };

            self.update_osl(event);

            // Send report for updated osm_state modifiers
            if activate_on_keypress {
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else {
            match self.osm_state {
                OneShotState::Initial(cur_modifiers) | OneShotState::Single(cur_modifiers) => {
                    self.osm_state = OneShotState::Single(cur_modifiers);
                    let timeout = Timer::after(self.keymap.one_shot_timeout());
                    match select(timeout, self.keyboard_event_subscriber.next_message_pure()).await {
                        Either::First(_) => {
                            // Timeout fired. Guard against the select race where
                            // the timer is polled first and wins even though a key
                            // event arrived at the subscriber at the same instant.
                            if let Some(e) = self.keyboard_event_subscriber.try_next_message_pure() {
                                // A key event was pending — one-shot is consumed, not timed out
                                if self.unprocessed_events.push(e).is_err() {
                                    warn!("Unprocessed event queue is full, dropping event");
                                }
                            } else {
                                // Genuinely timed out with no pending key event
                                self.update_osl(event);
                                if osm_config.tap_on_timeout {
                                    // Send bare modifier tap before clearing state
                                    self.send_keyboard_report_with_resolved_modifiers(true).await;
                                    self.osm_state = OneShotState::None;
                                    self.send_keyboard_report_with_resolved_modifiers(false).await;
                                } else {
                                    self.osm_state = OneShotState::None;
                                    // Send release report because modifiers were held
                                    if activate_on_keypress {
                                        self.send_keyboard_report_with_resolved_modifiers(false).await;
                                    }
                                }
                            }
                        }
                        Either::Second(e) => {
                            // New event, send it to queue
                            if self.unprocessed_events.push(e).is_err() {
                                warn!("Unprocessed event queue is full, dropping event");
                            }
                        }
                    }
                }
                OneShotState::Held(cur_modifiers) => {
                    let was_active = cur_modifiers & new_modifiers == new_modifiers;

                    if !was_active {
                        return;
                    }

                    // Release modifier
                    self.update_osl(event);
                    self.osm_state = OneShotState::None;

                    // This sends a separate hid report with the
                    // currently registered modifiers except the
                    // one shot modifiers -> this way "releasing" them.
                    self.send_keyboard_report_with_resolved_modifiers(false).await;
                }
                _ => (),
            };
        }
    }

    pub(crate) async fn process_action_osl(&mut self, layer_num: u8, event: KeyboardEvent) {
        // Update one shot state
        if event.pressed {
            // Deactivate old layer if any
            if let Some(&l) = self.osl_state.value() {
                self.keymap.deactivate_layer(l);
            }

            // Update layer of one shot
            self.osl_state = match self.osl_state {
                OneShotState::None => OneShotState::Initial(layer_num),
                OneShotState::Initial(_) => OneShotState::Initial(layer_num),
                OneShotState::Single(_) => OneShotState::Single(layer_num),
                OneShotState::Held(_) => OneShotState::Held(layer_num),
            };

            // Activate new layer
            self.keymap.activate_layer(layer_num);
        } else {
            match self.osl_state {
                OneShotState::Initial(l) | OneShotState::Single(l) => {
                    self.osl_state = OneShotState::Single(l);

                    let timeout = embassy_time::Timer::after(self.keymap.one_shot_timeout());
                    match select(timeout, self.keyboard_event_subscriber.next_message_pure()).await {
                        Either::First(_) => {
                            // Timeout, deactivate layer
                            self.keymap.deactivate_layer(layer_num);
                            self.osl_state = OneShotState::None;
                        }
                        Either::Second(e) => {
                            // New event, send it to queue
                            if self.unprocessed_events.push(e).is_err() {
                                warn!("Unprocessed event queue is full, dropping event");
                            }
                        }
                    }
                }
                OneShotState::Held(layer_num) => {
                    self.osl_state = OneShotState::None;
                    self.keymap.deactivate_layer(layer_num);
                }
                _ => (),
            };
        }
    }

    pub(crate) fn update_osm(&mut self, _event: KeyboardEvent) {
        match self.osm_state {
            OneShotState::Initial(m) => self.osm_state = OneShotState::Held(m),
            OneShotState::Single(_) => {
                // Once any key is pressed or released after an OSM tap,
                // the one-shot is consumed. On press, the modifier was already
                // included in the HID report (resolve_modifiers runs before
                // update_osm), so clearing here is safe and prevents the
                // state from lingering as Single between key press and release.
                self.osm_state = OneShotState::None;
            }
            _ => (),
        }
    }

    pub(crate) fn update_osl(&mut self, event: KeyboardEvent) {
        match self.osl_state {
            OneShotState::Initial(l) => self.osl_state = OneShotState::Held(l),
            OneShotState::Single(layer_num) => {
                if !event.pressed {
                    self.keymap.deactivate_layer(layer_num);
                    self.osl_state = OneShotState::None;
                }
            }
            _ => (),
        }
    }
}
