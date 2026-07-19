//! StickyKey action implementation.
//!
//! A unified one-shot action engine covering pure-mod (OSM), tap-key, and layer (OSL) shapes.
//! The shape is determined by the `StickyKeyAction` payload at compile time.
//! Runtime state and its lifecycle are represented by `StickyKeyState`.
//!
//! Timeout is driven solely by the run-loop deadline race (see `Keyboard::run`); there is
//! no inline `select` in this module. On expiry the run loop calls
//! [`Keyboard::release_sticky_key_if_active`].

use embassy_time::{Duration, Instant};
use rmk_types::action::StickyKeyAction;
use rmk_types::keycode::HidKeyCode;
use rmk_types::modifier::ModifierCombination;

use crate::config::StickyKeyReleaseMode;
use crate::event::{KeyboardEvent, KeyboardEventPos};
use crate::keyboard::Keyboard;
use crate::keymap::StickyKeyShape;

fn deadline_from_timeout(timeout: Duration) -> Option<Instant> {
    (timeout != Duration::MAX).then(|| Instant::now() + timeout)
}

/// The operation performed while a Sticky Key is active.
#[derive(Clone, Copy, Debug)]
enum StickyKeyEffect {
    Modifier,
    Layer(u8),
    TapKey(HidKeyCode),
}

/// Data carried through each active Sticky Key lifecycle state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ActiveStickyKey {
    /// Physical key that owns this latch.
    source: KeyboardEventPos,
    mods: ModifierCombination,
    effect: StickyKeyEffect,
    /// Selected Sticky Key profile (`u8::MAX` means default profile).
    profile: u8,
    repeat_count: u16,
    deadline: Option<Instant>,
}

/// Lifecycle of a Sticky Key.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum StickyKeyState {
    /// No Sticky Key is active.
    #[default]
    None,
    /// The physical Sticky Key is down and no foreign key has been pressed.
    Pressed(ActiveStickyKey),
    /// The physical Sticky Key was released and is armed for a foreign key.
    Latched(ActiveStickyKey),
    /// A foreign key was pressed while the physical Sticky Key remained down.
    Held(ActiveStickyKey),
}

enum ReleaseTransition {
    Ignored,
    Latched,
    Held,
}

impl StickyKeyState {
    pub fn value(&self) -> Option<&ModifierCombination> {
        self.active().map(|active| &active.mods)
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, StickyKeyState::None)
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.active().and_then(|active| active.deadline)
    }

    pub fn is_pure_mod(&self) -> bool {
        self.active()
            .is_some_and(|active| matches!(active.effect, StickyKeyEffect::Modifier))
    }

    pub fn is_tap_key(&self) -> bool {
        self.active()
            .is_some_and(|active| matches!(active.effect, StickyKeyEffect::TapKey(_)))
    }

    pub fn is_layer(&self) -> bool {
        self.active()
            .is_some_and(|active| matches!(active.effect, StickyKeyEffect::Layer(_)))
    }

    pub(crate) fn profile(&self) -> Option<u8> {
        self.active().map(|active| active.profile)
    }

    pub(crate) fn shape(&self) -> Option<StickyKeyShape> {
        if self.is_pure_mod() {
            Some(StickyKeyShape::PureMod)
        } else if self.is_layer() {
            Some(StickyKeyShape::Layer)
        } else if self.is_tap_key() {
            Some(StickyKeyShape::TapKey)
        } else {
            None
        }
    }

    pub(crate) fn is_held(&self) -> bool {
        matches!(self, Self::Held(_))
    }

    fn active(&self) -> Option<&ActiveStickyKey> {
        match self {
            Self::Pressed(active) | Self::Latched(active) | Self::Held(active) => Some(active),
            Self::None => None,
        }
    }
}

impl Keyboard<'_> {
    fn transition_on_release(
        &mut self,
        owner: Option<KeyboardEventPos>,
        deadline: Option<Instant>,
    ) -> ReleaseTransition {
        match self.sticky_key_state {
            StickyKeyState::Pressed(mut active) if owner.is_none_or(|owner| active.source == owner) => {
                active.deadline = deadline;
                self.sticky_key_state = StickyKeyState::Latched(active);
                ReleaseTransition::Latched
            }
            StickyKeyState::Held(active) if owner.is_none_or(|owner| active.source == owner) => {
                self.sticky_key_state = StickyKeyState::None;
                ReleaseTransition::Held
            }
            _ => ReleaseTransition::Ignored,
        }
    }

    pub(crate) async fn release_sticky_key_on_layer_event(&mut self, event: StickyKeyReleaseMode) {
        let (Some(index), Some(shape)) = (self.sticky_key_state.profile(), self.sticky_key_state.shape()) else {
            return;
        };
        if self
            .keymap
            .sticky_key_profile(index, shape)
            .release_mode
            .is_some_and(|mode| mode.contains(event))
        {
            self.release_sticky_key_if_active().await;
        }
    }

    pub(crate) async fn process_action_sticky_key(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
        let shape = if params.layer.is_some() {
            StickyKeyShape::Layer
        } else if params.key == HidKeyCode::No {
            StickyKeyShape::PureMod
        } else {
            StickyKeyShape::TapKey
        };

        if event.pressed
            && matches!(
                self.sticky_key_state,
                StickyKeyState::Latched(ActiveStickyKey { source, .. }) if source == event.pos
            )
            && self
                .keymap
                .sticky_key_profile(params.profile, shape)
                .release_mode
                .is_some_and(|mode| mode.double_tap())
        {
            self.release_sticky_key_if_active().await;
            return;
        }

        match shape {
            StickyKeyShape::Layer => self.process_sticky_layer(params, event).await,
            StickyKeyShape::PureMod => self.process_sticky_pure_mod(params, event).await,
            StickyKeyShape::TapKey => self.process_sticky_tap_key(params, event).await,
        }
    }

    /// Pure-mod (OSM) shape: accumulate the modifier across taps, apply it through the
    /// terminating key, honor `activate_on_keypress`/`quick_release`.
    async fn process_sticky_pure_mod(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
        let profile = self.keymap.sticky_key_profile(params.profile, StickyKeyShape::PureMod);
        let deadline = deadline_from_timeout(profile.timeout);

        if event.pressed {
            if self.sticky_key_state.is_active()
                && !self.sticky_key_state.is_pure_mod()
                && !self.sticky_key_state.is_layer()
            {
                self.release_sticky_key_if_active().await;
            }

            self.sticky_key_state = match self.sticky_key_state.active().copied() {
                None => StickyKeyState::Pressed(ActiveStickyKey {
                    source: event.pos,
                    mods: params.keep,
                    effect: StickyKeyEffect::Modifier,
                    profile: params.profile,
                    repeat_count: 1,
                    deadline,
                }),
                Some(mut active) => {
                    active.source = event.pos;
                    active.mods |= params.keep;
                    active.profile = params.profile;
                    active.deadline = deadline;
                    StickyKeyState::Pressed(active)
                }
            };

            if profile.activate_on_keypress {
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else {
            // Combo outputs may be released by a different constituent position,
            // so modifier actions cannot require the original source position.
            if matches!(self.transition_on_release(None, deadline), ReleaseTransition::Held) {
                self.send_keyboard_report_with_resolved_modifiers(false).await;
            }
        }
    }

    /// Layer (OSL) shape: activate the layer for the next foreign key. The layer carries
    /// no modifier, so consuming it emits no HID report.
    async fn process_sticky_layer(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
        let layer_num = params.layer.expect("layer shape requires a layer");
        let profile = self.keymap.sticky_key_profile(params.profile, StickyKeyShape::Layer);
        let deadline = deadline_from_timeout(profile.timeout);

        if event.pressed {
            if self.sticky_key_state.is_tap_key() {
                self.release_sticky_key_if_active().await;
            }

            let existing_mods = match self.sticky_key_state.active().copied() {
                Some(active) => {
                    if let StickyKeyEffect::Layer(previous_layer) = active.effect {
                        self.keymap.deactivate_layer(previous_layer);
                    }
                    active.mods
                }
                None => ModifierCombination::new(),
            };

            self.keymap.activate_layer(layer_num);
            self.sticky_key_state = StickyKeyState::Pressed(ActiveStickyKey {
                source: event.pos,
                mods: existing_mods | params.keep,
                effect: StickyKeyEffect::Layer(layer_num),
                profile: params.profile,
                repeat_count: 1,
                deadline,
            });
        } else {
            if matches!(
                self.transition_on_release(Some(event.pos), deadline),
                ReleaseTransition::Held
            ) {
                self.keymap.deactivate_layer(layer_num);
            }
        }
    }

    /// Tap-key (alt-tab) shape: send `keep` mods + `key` on every press, hold the mods
    /// between presses, cycle on each press (`max_repeat`). Ignores
    /// `activate_on_keypress`/`quick_release`.
    async fn process_sticky_tap_key(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
        let profile = self.keymap.sticky_key_profile(params.profile, StickyKeyShape::TapKey);
        let deadline = deadline_from_timeout(profile.timeout);

        if event.pressed {
            let is_different_tap_key = self
                .sticky_key_state
                .active()
                .is_some_and(|active| active.source != event.pos);
            if self.sticky_key_state.is_active() && (!self.sticky_key_state.is_tap_key() || is_different_tap_key) {
                self.release_sticky_key_if_active().await;
            }

            let mut should_deactivate = false;
            self.sticky_key_state = match self.sticky_key_state.active().copied() {
                None => StickyKeyState::Pressed(ActiveStickyKey {
                    source: event.pos,
                    mods: params.keep,
                    effect: StickyKeyEffect::TapKey(params.key),
                    profile: params.profile,
                    repeat_count: 1,
                    deadline,
                }),
                Some(mut active) => {
                    active.repeat_count = active.repeat_count.saturating_add(1);
                    if profile.max_repeat > 0 && active.repeat_count > profile.max_repeat {
                        should_deactivate = true;
                        StickyKeyState::None
                    } else {
                        active.deadline = deadline;
                        StickyKeyState::Pressed(active)
                    }
                }
            };

            if should_deactivate {
                self.send_keyboard_report_with_resolved_modifiers(false).await;
            } else {
                self.register_key(params.key, event);
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else if let StickyKeyState::Pressed(mut active) = self.sticky_key_state
            && active.source == event.pos
        {
            if active.deadline.is_none() {
                active.deadline = deadline;
            }
            self.unregister_key(params.key, event);
            self.send_keyboard_report_with_resolved_modifiers(false).await;
            self.sticky_key_state = StickyKeyState::Latched(active);
        }
    }

    /// Foreign-key hook for the pure-mod shape, mirroring the former `update_osm`.
    /// Called from `process_action_key` for every basic key. Drives the OSM-style
    /// phase transitions on the terminating key and returns `true` when the latch was
    /// consumed (so the caller can emit a quick-release report).
    ///
    /// Tap-key shape is untouched here — it is consumed elsewhere.
    ///
    /// Called only from `process_action_key` (basic keys), so a bare `Action::Modifier`
    /// no longer consumes a latched OSL the way the former `update_osl` did from the
    /// modifier path — only a non-modifier key, a layer change, or timeout consumes it.
    /// This narrowing is intentional (a held modifier is not a "terminating key") and
    /// matches how tap-key SKs already ignore bare modifiers.
    pub(crate) fn update_sticky_key(&mut self, event: KeyboardEvent) -> bool {
        if !self.sticky_key_state.is_pure_mod() && !self.sticky_key_state.is_layer() {
            return false;
        }
        let mode = self
            .sticky_key_state
            .profile()
            .zip(self.sticky_key_state.shape())
            .and_then(|(index, shape)| self.keymap.sticky_key_profile(index, shape).release_mode);
        match self.sticky_key_state {
            StickyKeyState::Pressed(mut active) => {
                active.deadline = None;
                self.sticky_key_state = StickyKeyState::Held(active);
                false
            }
            StickyKeyState::Latched(active) => {
                let release_on_press =
                    event.pressed && mode.is_some_and(|mode| mode.contains(StickyKeyReleaseMode::OTHER_KEY_PRESS));
                let release_on_release = !event.pressed
                    && (mode.is_none()
                        || mode.is_some_and(|mode| mode.contains(StickyKeyReleaseMode::OTHER_KEY_RELEASE)));
                if !release_on_press && !release_on_release {
                    return false;
                }

                if let StickyKeyEffect::Layer(layer) = active.effect {
                    self.keymap.deactivate_layer(layer);
                    self.sticky_key_state = StickyKeyState::None;
                    release_on_press
                } else {
                    self.sticky_key_state = StickyKeyState::None;
                    true
                }
            }
            StickyKeyState::None | StickyKeyState::Held(_) => false,
        }
    }

    /// Release a StickyKey whose timeout has elapsed.
    ///
    /// A physical key release must still be able to observe the active state, so a timeout that
    /// fires while the key is held only clears its deadline. Explicit cleanup (for a replacement
    /// key or layer change) uses `release_sticky_key_if_active` and must not be deferred.
    pub(crate) async fn release_sticky_key_if_active_on_timeout(&mut self) {
        if !self.sticky_key_state.is_active() {
            return;
        }

        // If the SK is still physically held, the deadline fired but the
        // key hasn't been released yet. Don't clear the latch — the physical release
        // handler (process_sticky_*) will transition Held→None cleanly. For pure-mod,
        // the deadline was set on press (→ Held on any other key press), so this can
        // only happen when the key is held and idle. For layer and tap-key shapes, the
        // deadline fires in the same scenario.
        // Clear the deadline to avoid busy-looping on every iteration.
        if let StickyKeyState::Pressed(active) = &mut self.sticky_key_state {
            debug!(
                "StickyKey timeout fired while key is still held — clearing deadline, deferring to physical release"
            );
            active.deadline = None;
            return;
        }

        self.release_sticky_key_if_active().await;
    }

    pub(crate) async fn release_sticky_key_if_active(&mut self) {
        if !self.sticky_key_state.is_active() {
            return;
        }

        debug!("Releasing StickyKey");

        // Decide whether the release needs its own HID report. A report is only meaningful
        // when the sticky modifier was actually visible in the last report:
        //  - tap-key shape: the modifier is always live between presses → always report.
        //  - pure-mod shape: only when promoted to Held, or when `activate_on_keypress`
        //    emitted the modifier early. A bare Latched pure-mod that times out before any
        //    key (and without early activation) never emitted the modifier, so releasing it
        //    must NOT produce a spurious empty report. Mirrors the former OSM timeout path.
        //  - layer shape: deactivating a layer emits nothing → never report.
        let needs_report = if self.sticky_key_state.is_pure_mod() {
            let activate_on_keypress = self.sticky_key_state.profile().is_some_and(|index| {
                self.keymap
                    .sticky_key_profile(index, StickyKeyShape::PureMod)
                    .activate_on_keypress
            });
            self.sticky_key_state.is_held() || activate_on_keypress
        } else {
            // tap-key shape always reports; layer shape never does (deactivating emits nothing).
            !self.sticky_key_state.is_layer()
        };

        // A tap-key may still have its HID key registered when it is displaced by a different
        // StickyKey while physically held. Unregister it before clearing the latch so it cannot
        // remain stuck in the report.
        if let Some(ActiveStickyKey {
            effect: StickyKeyEffect::TapKey(hid_key),
            source,
            ..
        }) = self.sticky_key_state.active().copied()
        {
            self.unregister_key(
                hid_key,
                KeyboardEvent {
                    pressed: false,
                    pos: source,
                },
            );
        }

        // For the layer shape, deactivate the active layer before clearing the latch.
        if let Some(ActiveStickyKey {
            effect: StickyKeyEffect::Layer(layer_num),
            ..
        }) = self.sticky_key_state.active().copied()
        {
            self.keymap.deactivate_layer(layer_num);
        }

        self.sticky_key_state = StickyKeyState::None;
        if needs_report {
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }
}
