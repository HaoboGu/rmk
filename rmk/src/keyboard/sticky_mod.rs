//! StickyMod action implementation
//!
//! StickyMod provides Alt+Tab-like window/tab switching behavior.
//! On first press: sends modifier + key. On release: holds modifier.
//! Subsequent presses send only the key. Modifier releases when any
//! non-SM/non-modifier key is pressed, or when the layer changes.

use rmk_types::keycode::KeyCode;
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;

/// State for StickyMod action
#[derive(Default, Debug)]
pub(crate) enum StickyModState {
    /// StickyMod is inactive
    #[default]
    None,
    /// StickyMod is active — modifier is being held
    Active(ModifierCombination),
}

impl StickyModState {
    /// Get the held modifiers if StickyMod is active
    pub fn value(&self) -> Option<&ModifierCombination> {
        match self {
            StickyModState::Active(mods) => Some(mods),
            StickyModState::None => None,
        }
    }

    /// Check if StickyMod is currently active
    pub fn is_active(&self) -> bool {
        matches!(self, StickyModState::Active(_))
    }
}

impl Keyboard<'_> {
    /// Process StickyMod action
    ///
    /// Flow:
    /// - First press: activate SM state, register modifier + key, send report
    /// - Release: unregister key, keep modifier held (via state + resolve_explicit_modifiers)
    /// - Subsequent press: key already held by state, just register key again
    /// - Subsequent release: unregister key, modifier stays
    /// - Any non-SM/non-modifier key press: release_sticky_mod_if_active() called before processing
    /// - Layer change: release_sticky_mod_if_active() called as cleanup
    pub(crate) async fn process_action_sticky_mod(
        &mut self,
        key: KeyCode,
        modifiers: ModifierCombination,
        event: KeyboardEvent,
    ) {
        if event.pressed {
            // Activate SM if not already active (first press)
            // If already active with same or different modifier, keep current state
            // (different SM key while active: first SM was already released by the
            //  any-key-press check in process_key_action_normal before we get here,
            //  so this is always a fresh activation)
            if let StickyModState::None = self.sticky_mod_state {
                self.sticky_mod_state = StickyModState::Active(modifiers);
            }

            // Register the key (e.g., Tab) — modifier comes from resolve_explicit_modifiers
            if let KeyCode::Hid(hid_key) = key {
                self.register_key(hid_key, event);
            }
            self.send_keyboard_report_with_resolved_modifiers(true).await;
        } else {
            // Release the key, modifier stays held via StickyModState::Active
            if let KeyCode::Hid(hid_key) = key {
                self.unregister_key(hid_key, event);
            }
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }

    /// Release StickyMod if active. Called when:
    /// - A non-SM, non-modifier key is pressed
    /// - A layer is deactivated
    /// - Timeout expires (Phase 2)
    pub(crate) async fn release_sticky_mod_if_active(&mut self) {
        if self.sticky_mod_state.is_active() {
            debug!("Releasing StickyMod");
            self.sticky_mod_state = StickyModState::None;
            // Send report to reflect modifier release
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }
}
