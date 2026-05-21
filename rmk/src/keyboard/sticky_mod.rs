//! StickyMod action implementation
//!
//! StickyMod provides Alt+Tab-like window/tab switching behavior.
//! On first press: sends modifier + key. On release: holds modifier.
//! Subsequent presses send only the key. Modifier releases when any
//! non-SM/non-modifier key is pressed, or when the layer changes,
//! or when the optional timeout fires from the main event loop.

use embassy_time::{Duration, Instant};
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
    /// StickyMod is active — modifier is held, optional deadline for auto-release
    Active {
        mods: ModifierCombination,
        /// When to auto-release the modifier. None = no timeout.
        deadline: Option<Instant>,
    },
}

impl StickyModState {
    /// Get the held modifiers if StickyMod is active
    pub fn value(&self) -> Option<&ModifierCombination> {
        match self {
            StickyModState::Active { mods, .. } => Some(mods),
            StickyModState::None => None,
        }
    }

    /// Check if StickyMod is currently active
    pub fn is_active(&self) -> bool {
        matches!(self, StickyModState::Active { .. })
    }

    /// Return the auto-release deadline if one is set, for use in the main event loop
    pub fn deadline(&self) -> Option<Instant> {
        match self {
            StickyModState::Active { deadline, .. } => *deadline,
            StickyModState::None => None,
        }
    }
}

impl Keyboard<'_> {
    /// Process StickyMod action
    ///
    /// Flow:
    /// - First press: activate SM state with deadline, register modifier + key, send report
    /// - Subsequent press: reset deadline, register key again
    /// - Release: unregister key, modifier stays held (deadline unchanged)
    /// - Timeout: fires from `run()` loop via `sticky_mod_state.deadline()`
    /// - Any non-SM/non-modifier key press: release_sticky_mod_if_active() called before processing
    /// - Layer change: release_sticky_mod_if_active() called as cleanup
    pub(crate) async fn process_action_sticky_mod(
        &mut self,
        key: KeyCode,
        modifiers: ModifierCombination,
        event: KeyboardEvent,
    ) {
        if event.pressed {
            let timeout = self.keymap.sticky_mod_timeout();
            let deadline = (timeout != Duration::MAX).then(|| Instant::now() + timeout);

            match &mut self.sticky_mod_state {
                StickyModState::None => {
                    self.sticky_mod_state = StickyModState::Active {
                        mods: modifiers,
                        deadline,
                    };
                }
                StickyModState::Active { deadline: d, .. } => {
                    // Reset deadline on each SM press (timeout counts from last press)
                    *d = deadline;
                }
            }

            if let KeyCode::Hid(hid_key) = key {
                self.register_key(hid_key, event);
            }
            self.send_keyboard_report_with_resolved_modifiers(true).await;
        } else {
            // Release the key; modifier stays held via StickyModState::Active.
            // Deadline remains — the run() loop fires auto-release when it expires.
            if let KeyCode::Hid(hid_key) = key {
                self.unregister_key(hid_key, event);
            }
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }

    /// Release StickyMod if active. Called when:
    /// - A non-SM, non-modifier key is pressed
    /// - A layer is deactivated
    /// - Timeout deadline fires in the main event loop
    pub(crate) async fn release_sticky_mod_if_active(&mut self) {
        if self.sticky_mod_state.is_active() {
            debug!("Releasing StickyMod");
            self.sticky_mod_state = StickyModState::None;
            // Send report to reflect modifier release
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }
}
