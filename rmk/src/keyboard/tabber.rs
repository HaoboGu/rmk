//! Tabber action implementation
//!
//! Tabber provides Alt+Tab-like window switching behavior by holding a modifier
//! while repeatedly tapping Tab. The modifier is held across multiple key presses
//! until the layer changes.

use rmk_types::keycode::HidKeyCode;
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;

/// State for Tabber action
#[derive(Default)]
pub(crate) enum TabberState<T> {
    /// Tabber is active and holding modifiers
    Active(T),
    /// Tabber is inactive
    #[default]
    None,
}

impl<T> TabberState<T> {
    /// Get the held modifiers if Tabber is active
    pub fn value(&self) -> Option<&T> {
        match self {
            TabberState::Active(mods) => Some(mods),
            TabberState::None => None,
        }
    }
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    /// Process Tabber action
    ///
    /// Mechanism:
    /// - First press: Send Modifier + Tab
    /// - First release: Release Tab, keep modifier held (via TabberState)
    /// - Subsequent presses: Send Tab only (modifier stays held via resolve_explicit_modifiers)
    /// - Subsequent releases: Release Tab only (modifier stays held via resolve_explicit_modifiers)
    /// - Layer change: Cleanup releases all Tabber-held modifiers
    /// - Shift integration: If Shift is held when Tabber is pressed, add Shift to Tab
    pub(crate) async fn process_action_tabber(&mut self, modifiers: ModifierCombination, event: KeyboardEvent) {
        // Safety check: Tabber cannot be used in base layer (layer 0)
        // This is essential because layer 0 has no mechanism to trigger cleanup
        let current_layer = self.keymap.borrow().get_activated_layer();
        if current_layer == 0 {
            warn!("Tabber action cannot be used in base layer (layer 0)");
            return;
        }

        if event.pressed {
            // Key press
            if let TabberState::None = self.tabber_state {
                self.tabber_state = TabberState::Active(modifiers);
            }

            self.register_key(HidKeyCode::Tab, event);
            self.send_keyboard_report_with_resolved_modifiers(true).await;
        } else {
            // Key release
            if let TabberState::Active(_) = self.tabber_state {
                self.unregister_key(HidKeyCode::Tab, event);
                self.send_keyboard_report_with_resolved_modifiers(false).await;
            }
        }
    }

    /// Clean up Tabber state when layer changes
    ///
    /// This should be called after any layer deactivation to ensure
    /// Tabber-held modifiers are properly released.
    pub(crate) async fn cleanup_tabber_on_layer_change(&mut self) {
        if let TabberState::Active(_) = self.tabber_state {
            debug!("Cleaning up Tabber due to layer change");
            // Clear Tabber state
            self.tabber_state = TabberState::None;
            // Send a report to reflect the modifier release
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }
}
