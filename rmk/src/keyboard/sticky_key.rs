//! StickyKey action implementation.
//!
//! StickyKey holds a modifier combination across key presses for Alt+Tab-like cycling.
//! Features:
//! - `max_repeat`: limit fires before auto-release (0 = infinite)
//! - `timeout_ms`: per-key timeout override (0 = use global config)
//! - `exit_on_layer_change`: whether layer changes release the SK
//!
//! ## `max_repeat` semantics
//! count starts at 1 on first press. On each subsequent press, count is incremented.
//! Deactivation fires when count > max_repeat (strictly greater), so max_repeat=N fires
//! the key exactly N times and deactivates silently on press N+1.

use embassy_time::{Duration, Instant};
use rmk_types::action::StickyKeyAction;
use rmk_types::keycode::KeyCode;
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;

/// State for the StickyKey action.
#[derive(Default, Debug)]
pub(crate) enum StickyKeyState {
    /// StickyKey is inactive.
    #[default]
    None,
    /// StickyKey is active — modifiers held, optional deadline for auto-release.
    Active {
        mods: ModifierCombination,
        repeat_count: u16,
        max_repeat: u16,
        exit_on_layer_change: bool,
        deadline: Option<Instant>,
    },
}

impl StickyKeyState {
    pub fn value(&self) -> Option<&ModifierCombination> {
        match self {
            StickyKeyState::Active { mods, .. } => Some(mods),
            StickyKeyState::None => None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, StickyKeyState::Active { .. })
    }

    pub fn deadline(&self) -> Option<Instant> {
        match self {
            StickyKeyState::Active { deadline, .. } => *deadline,
            StickyKeyState::None => None,
        }
    }

    pub fn exit_on_layer_change(&self) -> bool {
        matches!(self, StickyKeyState::Active { exit_on_layer_change: true, .. })
    }
}

impl Keyboard<'_> {
    pub(crate) async fn process_action_sticky_key(
        &mut self,
        params: StickyKeyAction,
        event: KeyboardEvent,
    ) {
        if event.pressed {
            let timeout = if params.timeout_ms > 0 {
                Duration::from_millis(params.timeout_ms as u64)
            } else {
                self.keymap.sticky_key_timeout()
            };
            let deadline = (timeout != Duration::MAX).then(|| Instant::now() + timeout);

            let mut should_deactivate = false;

            match &mut self.sticky_key_state {
                StickyKeyState::None => {
                    self.sticky_key_state = StickyKeyState::Active {
                        mods: params.keep,
                        repeat_count: 1,
                        max_repeat: params.max_repeat,
                        exit_on_layer_change: params.exit_on_layer_change,
                        deadline,
                    };
                }
                StickyKeyState::Active {
                    repeat_count,
                    max_repeat: mr,
                    deadline: d,
                    ..
                } => {
                    *repeat_count += 1;
                    let count = *repeat_count;
                    let mr_val = *mr;
                    if mr_val > 0 && count > mr_val {
                        should_deactivate = true;
                    } else {
                        *d = deadline;
                    }
                }
            }

            if should_deactivate {
                self.sticky_key_state = StickyKeyState::None;
                self.send_keyboard_report_with_resolved_modifiers(false).await;
            } else {
                if let KeyCode::Hid(hid_key) = params.key {
                    self.register_key(hid_key, event);
                }
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else {
            if let KeyCode::Hid(hid_key) = params.key {
                self.unregister_key(hid_key, event);
            }
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }

    pub(crate) async fn release_sticky_key_if_active(&mut self) {
        if self.sticky_key_state.is_active() {
            debug!("Releasing StickyKey");
            self.sticky_key_state = StickyKeyState::None;
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }
}
