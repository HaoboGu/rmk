use rmk_types::keycode::HidKeyCode;
use rmk_types::modifier::ModifierCombination;

use crate::event::{KeyboardEvent, ModifierEvent, publish_event};

/// Keyboard state needed for normal key registration operations.
pub(crate) struct KeyReportState<'a> {
    pub held_keycodes: &'a mut [HidKeyCode; 6],
    pub registered_keys: &'a mut [Option<KeyboardEvent>; 6],
    pub held_modifiers: &'a mut ModifierCombination,
    pub fork_keep_mask: &'a mut ModifierCombination,
}

/// Trait defining low-level key press/release handling.
pub(crate) trait KeypressHandler {
    fn register_key(&mut self, key: HidKeyCode, event: KeyboardEvent, state: &mut KeyReportState);
    fn unregister_key(&mut self, key: HidKeyCode, event: KeyboardEvent, state: &mut KeyReportState);
}

/// Strategy for forwarding keyboard interactions to the host.
///
/// - `Normal`: forward HID reports to host as usual.
/// - `PasskeyEntry`: intercept/suppress host interactions while collecting passkey digits.
pub(crate) trait HostReportStrategy {
    fn should_send_report(&self) -> bool;
}

/// Normal keyboard mode — registers keys into the HID report.
pub(crate) struct NormalHandler;

impl KeypressHandler for NormalHandler {
    fn register_key(&mut self, key: HidKeyCode, event: KeyboardEvent, state: &mut KeyReportState) {
        if key.is_modifier() {
            *state.held_modifiers |= key.to_hid_modifiers();
            publish_event(ModifierEvent {
                modifier: *state.held_modifiers,
            });
            // If a modifier key arrives after fork activation, it should be kept
            *state.fork_keep_mask |= key.to_hid_modifiers();
        } else {
            // Find existing slot for this position
            let slot = state.registered_keys.iter().enumerate().find_map(|(i, k)| {
                if let Some(e) = k
                    && event.pos == e.pos
                {
                    return Some(i);
                }
                None
            });

            if let Some(index) = slot {
                state.held_keycodes[index] = key;
                state.registered_keys[index] = Some(event);
            } else if let Some(index) = state.held_keycodes.iter().position(|&k| k == HidKeyCode::No) {
                state.held_keycodes[index] = key;
                state.registered_keys[index] = Some(event);
            }
        }
    }

    fn unregister_key(&mut self, key: HidKeyCode, event: KeyboardEvent, state: &mut KeyReportState) {
        if key.is_modifier() {
            *state.held_modifiers &= !key.to_hid_modifiers();
            publish_event(ModifierEvent {
                modifier: *state.held_modifiers,
            });
        } else {
            let slot = state.registered_keys.iter().enumerate().find_map(|(i, k)| {
                if let Some(e) = k
                    && event.pos == e.pos
                {
                    return Some(i);
                }
                None
            });

            if let Some(index) = slot {
                state.held_keycodes[index] = HidKeyCode::No;
                state.registered_keys[index] = None;
            } else if let Some(index) = state.held_keycodes.iter().position(|&k| k == key) {
                state.held_keycodes[index] = HidKeyCode::No;
                state.registered_keys[index] = None;
            }
        }
    }

}

/// Passkey entry mode — collects digits instead of registering keys.
#[cfg(feature = "ble_passkey_entry")]
pub(crate) struct PasskeyHandler {
    passkey_state: crate::ble::passkey::PasskeyEntryState,
}

#[cfg(feature = "ble_passkey_entry")]
impl KeypressHandler for PasskeyHandler {
    fn register_key(&mut self, _key: HidKeyCode, _event: KeyboardEvent, _state: &mut KeyReportState) {
        // No-op: in passkey mode, key presses are not registered
    }

    fn unregister_key(&mut self, key: HidKeyCode, _event: KeyboardEvent, _state: &mut KeyReportState) {
        use crate::ble::passkey::{
            PASSKEY_RESPONSE, end_passkey_entry_session, hid_keycode_to_digit,
        };

        if let Some(digit) = hid_keycode_to_digit(key) {
            if self.passkey_state.add_digit(digit) {
                info!("[passkey] Digit entered, {}/6", self.passkey_state.digit_count());
            }
        } else if matches!(key, HidKeyCode::Enter | HidKeyCode::KpEnter) {
            if self.passkey_state.is_complete() {
                let passkey = self.passkey_state.to_passkey();
                info!("[passkey] Submitting passkey");
                PASSKEY_RESPONSE.signal(Some(passkey));
                self.passkey_state.reset();
            } else {
                warn!(
                    "[passkey] Enter pressed but only {}/6 digits entered",
                    self.passkey_state.digit_count()
                );
            }
        } else if matches!(key, HidKeyCode::Escape) {
            info!("[passkey] Cancelled");
            end_passkey_entry_session();
            PASSKEY_RESPONSE.signal(None);
            self.passkey_state.reset();
        } else if matches!(key, HidKeyCode::Backspace) {
            if self.passkey_state.remove_digit() {
                info!("[passkey] Backspace, {}/6 digits", self.passkey_state.digit_count());
            }
        }
        // All other keys are silently consumed
    }

}

/// Active keyboard mode — delegates to the appropriate handler.
pub(crate) enum KeyboardMode {
    Normal(NormalHandler),
    #[cfg(feature = "ble_passkey_entry")]
    Passkey(PasskeyHandler),
}

impl KeyboardMode {
    /// Check if currently in passkey entry mode.
    pub fn is_passkey(&self) -> bool {
        match self {
            KeyboardMode::Normal(_) => false,
            #[cfg(feature = "ble_passkey_entry")]
            KeyboardMode::Passkey(_) => true,
        }
    }

    /// Switch to passkey entry mode.
    #[cfg(feature = "ble_passkey_entry")]
    pub fn enter_passkey_mode(&mut self) {
        *self = KeyboardMode::Passkey(PasskeyHandler {
            passkey_state: crate::ble::passkey::PasskeyEntryState::new(),
        });
    }

    /// Switch to normal mode.
    #[cfg(feature = "ble_passkey_entry")]
    pub fn enter_normal_mode(&mut self) {
        *self = KeyboardMode::Normal(NormalHandler);
    }
}

impl KeypressHandler for KeyboardMode {
    fn register_key(&mut self, key: HidKeyCode, event: KeyboardEvent, state: &mut KeyReportState) {
        match self {
            KeyboardMode::Normal(h) => h.register_key(key, event, state),
            #[cfg(feature = "ble_passkey_entry")]
            KeyboardMode::Passkey(h) => h.register_key(key, event, state),
        }
    }

    fn unregister_key(&mut self, key: HidKeyCode, event: KeyboardEvent, state: &mut KeyReportState) {
        match self {
            KeyboardMode::Normal(h) => h.unregister_key(key, event, state),
            #[cfg(feature = "ble_passkey_entry")]
            KeyboardMode::Passkey(h) => h.unregister_key(key, event, state),
        }
    }

}

impl HostReportStrategy for NormalHandler {
    fn should_send_report(&self) -> bool {
        true
    }
}

#[cfg(feature = "ble_passkey_entry")]
impl HostReportStrategy for PasskeyHandler {
    fn should_send_report(&self) -> bool {
        false
    }
}

impl HostReportStrategy for KeyboardMode {
    fn should_send_report(&self) -> bool {
        match self {
            KeyboardMode::Normal(h) => h.should_send_report(),
            #[cfg(feature = "ble_passkey_entry")]
            KeyboardMode::Passkey(h) => h.should_send_report(),
        }
    }
}
