//! StickyKey action implementation.
//!
//! A unified one-shot action engine covering pure-mod (OSM), tap-key, and layer (OSL) shapes.
//! The shape is determined by the `StickyKeyAction` payload at compile time.
//! Runtime state is tracked in `StickyKeyState`; the latch phase is tracked in `SkPhase`.

use embassy_time::{Duration, Instant};
use rmk_types::action::StickyKeyAction;
use rmk_types::keycode::{HidKeyCode, KeyCode};
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;
use crate::keyboard::Keyboard;

/// Latch phase of a sticky key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum SkPhase {
    /// SK pressed, not yet consumed.
    #[default]
    Pressed,
    /// Armed — waiting for the next (foreign) key.
    Latched,
    /// Promoted to held (key released after another key was used).
    Held,
}

/// State for the StickyKey action.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) enum StickyKeyState {
    /// StickyKey is inactive.
    #[default]
    None,
    /// StickyKey is active — carries all latch state the engine needs.
    Active {
        mods: ModifierCombination,
        /// `KeyCode::Hid(HidKeyCode::No)` = pure-mod or layer shape; any other key = tap-key shape.
        key: KeyCode,
        /// `Some(n)` = OSL shape; `None` = pure-mod or tap-key shape.
        layer: Option<u8>,
        phase: SkPhase,
        repeat_count: u16,
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

    /// True when this is a pure-mod shape: active with no tap key and no layer.
    pub fn is_pure_mod(&self) -> bool {
        matches!(
            self,
            StickyKeyState::Active {
                key: KeyCode::Hid(HidKeyCode::No),
                layer: None,
                ..
            }
        )
    }

    /// True when this is a tap-key shape: active with a non-No key code.
    pub fn is_tap_key(&self) -> bool {
        self.is_active() && !self.is_pure_mod() && !self.is_layer()
    }

    /// True when this is a layer (OSL) shape: active with a `Some` layer.
    pub fn is_layer(&self) -> bool {
        matches!(self, StickyKeyState::Active { layer: Some(_), .. })
    }
}

impl Keyboard<'_> {
    pub(crate) async fn process_action_sticky_key(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
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
                    if *mr > 0 && *repeat_count > *mr {
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
            // Only unregister and report if SK was active (key was registered on press).
            // If max_repeat deactivated SK silently on the press event, the key was never
            // registered, so the release is a no-op.
            if self.sticky_key_state.is_active() {
                if let KeyCode::Hid(hid_key) = params.key {
                    self.unregister_key(hid_key, event);
                }
                self.send_keyboard_report_with_resolved_modifiers(false).await;
            }
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
