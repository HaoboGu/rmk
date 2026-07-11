//! StickyKey action implementation.
//!
//! A unified one-shot action engine covering pure-mod (OSM), tap-key, and layer (OSL) shapes.
//! The shape is determined by the `StickyKeyAction` payload at compile time.
//! Runtime state is tracked in `StickyKeyState`; the latch phase is tracked in `SkPhase`.
//!
//! Timeout is driven solely by the run-loop deadline race (see `Keyboard::run`); there is
//! no inline `select` in this module. On expiry the run loop calls
//! [`Keyboard::release_sticky_key_if_active`].

use embassy_time::{Duration, Instant};
use rmk_types::action::StickyKeyAction;
use rmk_types::keycode::{HidKeyCode, KeyCode};
use rmk_types::modifier::ModifierCombination;

use crate::event::{KeyboardEvent, KeyboardEventPos};
use crate::keyboard::Keyboard;

/// Latch phase of a sticky key.
///
/// Mirrors the former OSM state machine: `Pressed` == Initial, `Latched` == Single,
/// `Held` == Held.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum SkPhase {
    /// SK pressed, not yet consumed (still physically held). OSM `Initial`.
    #[default]
    Pressed,
    /// Armed — SK released before any other key, waiting for the next (foreign) key. OSM `Single`.
    Latched,
    /// Another key was pressed while the SK was still held; behaves like a normal held
    /// modifier until the SK is released. OSM `Held`.
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
        /// Physical key that owns this latch.
        source: KeyboardEventPos,
        mods: ModifierCombination,
        /// `KeyCode::Hid(HidKeyCode::No)` = pure-mod or layer shape; any other key = tap-key shape.
        key: KeyCode,
        /// `Some(n)` = OSL shape; `None` = pure-mod or tap-key shape.
        layer: Option<u8>,
        phase: SkPhase,
        /// Whether the physical StickyKey switch is currently held down.
        pressed: bool,
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
        if params.layer.is_some() {
            self.process_sticky_layer(params, event).await;
        } else if params.key == KeyCode::Hid(HidKeyCode::No) {
            self.process_sticky_pure_mod(params, event).await;
        } else {
            self.process_sticky_tap_key(params, event).await;
        }
    }

    /// Pure-mod (OSM) shape: accumulate the modifier across taps, apply it through the
    /// terminating key, honor `activate_on_keypress`/`quick_release`.
    async fn process_sticky_pure_mod(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
        let config = self.keymap.sticky_key_config();
        let deadline = (config.timeout != Duration::MAX).then(|| Instant::now() + config.timeout);

        if event.pressed {
            // Single mutually-exclusive latch: a pure-mod press accumulates (3c) onto an
            // existing pure-mod latch, but REPLACES any other shape (layer or tap-key).
            // Releasing the foreign latch first deactivates a held layer and drops its mods
            // cleanly, so only a same-shape (pure-mod) latch can reach the accumulate arm below.
            if self.sticky_key_state.is_active()
                && !self.sticky_key_state.is_pure_mod()
                && !self.sticky_key_state.is_layer()
            {
                self.release_sticky_key_if_active().await;
            }
            match &mut self.sticky_key_state {
                StickyKeyState::None => {
                    self.sticky_key_state = StickyKeyState::Active {
                        source: event.pos,
                        mods: params.keep,
                        key: params.key,
                        layer: None,
                        phase: SkPhase::Pressed,
                        pressed: true,
                        repeat_count: 1,
                        deadline,
                    };
                }
                StickyKeyState::Active {
                    mods,
                    pressed,
                    deadline: d,
                    ..
                } => {
                    // Same-shape pure-mod re-press: accumulate (3c) and refresh the deadline.
                    *mods |= params.keep;
                    *pressed = true;
                    *d = deadline;
                }
            }

            if config.activate_on_keypress {
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else {
            // SK released.
            if let StickyKeyState::Active { pressed, .. } = &mut self.sticky_key_state {
                *pressed = false;
            }
            match self.sticky_key_state {
                StickyKeyState::Active {
                    phase: SkPhase::Pressed,
                    ..
                } => {
                    // Released before any other key → arm it for the next key. Refresh the
                    // deadline on release-to-Latched so the timeout is measured from release
                    // time (not press time). Mirrors the layer shape behavior at line 214.
                    if let StickyKeyState::Active { phase, deadline: d, .. } = &mut self.sticky_key_state {
                        *phase = SkPhase::Latched;
                        *d = deadline;
                    }
                }
                StickyKeyState::Active {
                    phase: SkPhase::Held, ..
                } => {
                    // Held-mode: the modifier was applied as a normal held modifier; releasing
                    // the SK releases it now in its own report.
                    self.sticky_key_state = StickyKeyState::None;
                    self.send_keyboard_report_with_resolved_modifiers(false).await;
                }
                _ => {}
            }
        }
    }

    /// Layer (OSL) shape: activate the layer for the next foreign key. Mirrors the former
    /// `process_action_osl`. The layer carries no modifier, so consuming it emits no HID
    /// report — the foreign key resolves on the active layer in `process_action_key` before
    /// the latch is consumed.
    async fn process_sticky_layer(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
        let layer_num = params.layer.expect("layer shape requires a layer");
        let config = self.keymap.sticky_key_config();
        let deadline = (config.timeout != Duration::MAX).then(|| Instant::now() + config.timeout);

        if event.pressed {
            // Latch-replacement rule on a single mutually-exclusive latch: a layer SK press
            // takes over the latch. Deactivate any previously-latched OSL layer first, then
            // drop any latched mods/tap-key. A layer-on-layer press keeps the existing phase
            // (mirrors old `process_action_osl` lines 51-56); any other shape becomes a fresh
            // Pressed latch.
            let (prev_phase, existing_mods) = match self.sticky_key_state {
                StickyKeyState::Active {
                    layer: Some(prev_layer),
                    phase,
                    mods,
                    ..
                } => {
                    self.keymap.deactivate_layer(prev_layer);
                    (phase, mods)
                }
                StickyKeyState::Active { mods, phase, .. } => (phase, mods),
                _ => (SkPhase::Pressed, ModifierCombination::new()),
            };

            self.keymap.activate_layer(layer_num);
            self.sticky_key_state = StickyKeyState::Active {
                source: event.pos,
                mods: existing_mods | params.keep,
                key: params.key,
                layer: Some(layer_num),
                phase: prev_phase,
                pressed: true,
                repeat_count: 1,
                deadline,
            };
        } else {
            // SK released.
            if !matches!(
                self.sticky_key_state,
                StickyKeyState::Active { source, .. } if source == event.pos
            ) {
                return;
            }
            match self.sticky_key_state {
                StickyKeyState::Active {
                    phase: SkPhase::Pressed | SkPhase::Latched,
                    ..
                } => {
                    // Released before any other key → arm it for the next key and (re)arm the
                    // deadline so the run-loop race covers expiry.
                    if let StickyKeyState::Active {
                        phase,
                        pressed,
                        deadline: d,
                        ..
                    } = &mut self.sticky_key_state
                    {
                        *phase = SkPhase::Latched;
                        *pressed = false;
                        *d = deadline;
                    }
                }
                StickyKeyState::Active {
                    phase: SkPhase::Held, ..
                } => {
                    // Held-mode: the layer stayed active while the SK was physically held.
                    // Releasing the SK deactivates the layer now (no HID report).
                    self.keymap.deactivate_layer(layer_num);
                    self.sticky_key_state = StickyKeyState::None;
                }
                StickyKeyState::None => {}
            }
        }
    }

    /// Tap-key (alt-tab) shape: send `keep` mods + `key` on every press, hold the mods
    /// between presses, cycle on each press (`max_repeat`). Ignores
    /// `activate_on_keypress`/`quick_release`.
    async fn process_sticky_tap_key(&mut self, params: StickyKeyAction, event: KeyboardEvent) {
        let config = self.keymap.sticky_key_config();
        let deadline = (config.timeout != Duration::MAX).then(|| Instant::now() + config.timeout);

        if event.pressed {
            // A repeated press of the same physical tap-key cycles it. A different tap-key, like
            // any foreign StickyKey shape, replaces the active latch so it gets its own key and
            // modifiers rather than reusing the first key's state.
            let is_different_tap_key = matches!(
                self.sticky_key_state,
                StickyKeyState::Active { source, .. } if source != event.pos
            );
            if self.sticky_key_state.is_active() && (!self.sticky_key_state.is_tap_key() || is_different_tap_key) {
                self.release_sticky_key_if_active().await;
            }

            let mut should_deactivate = false;

            match &mut self.sticky_key_state {
                StickyKeyState::None => {
                    self.sticky_key_state = StickyKeyState::Active {
                        source: event.pos,
                        mods: params.keep,
                        key: params.key,
                        layer: None,
                        phase: SkPhase::Latched,
                        pressed: true,
                        repeat_count: 1,
                        deadline,
                    };
                }
                StickyKeyState::Active {
                    pressed,
                    repeat_count,
                    deadline: d,
                    ..
                } => {
                    // Saturating so an unbounded (`max_repeat == 0`) cycle can never overflow
                    // the counter and panic on a debug build after 65535 presses.
                    *repeat_count = repeat_count.saturating_add(1);
                    if config.max_repeat > 0 && *repeat_count > config.max_repeat {
                        should_deactivate = true;
                    } else {
                        *pressed = true;
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
            if let StickyKeyState::Active { source, pressed, .. } = &mut self.sticky_key_state
                && *source == event.pos
            {
                *pressed = false;
                if let KeyCode::Hid(hid_key) = params.key {
                    self.unregister_key(hid_key, event);
                }
                self.send_keyboard_report_with_resolved_modifiers(false).await;
            }
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
        // Layer (OSL) shape: mirror the former `update_osl`. Pressed→Held on a foreign key
        // (handled by the shared Pressed arm below, which also clears the deadline). A Latched
        // layer is consumed on the foreign key's RELEASE: deactivate the layer and clear the
        // latch. No HID report — deactivating a layer emits nothing.
        if let StickyKeyState::Active {
            phase: SkPhase::Latched,
            layer: Some(layer_num),
            ..
        } = self.sticky_key_state
        {
            if !event.pressed {
                self.keymap.deactivate_layer(layer_num);
                self.sticky_key_state = StickyKeyState::None;
            }
            return false;
        }
        let quick_release = self.keymap.sticky_key_config().quick_release;
        match &mut self.sticky_key_state {
            StickyKeyState::Active {
                phase: phase @ SkPhase::Pressed,
                deadline,
                ..
            } => {
                // A key was pressed while the SK is still physically held → promote to Held.
                // OSM `Held` has no timeout: the modifier stays live until the SK is physically
                // released (held-alt-tab use case). Clear the run-loop deadline so it does not
                // spuriously time-out while held.
                *phase = SkPhase::Held;
                *deadline = None;
                false
            }
            StickyKeyState::Active {
                phase: SkPhase::Latched,
                layer: Some(layer_num),
                ..
            } if quick_release && event.pressed => {
                self.keymap.deactivate_layer(*layer_num);
                self.sticky_key_state = StickyKeyState::None;
                true
            }
            StickyKeyState::Active {
                phase: SkPhase::Latched,
                ..
            } if quick_release && event.pressed => {
                self.sticky_key_state = StickyKeyState::None;
                true
            }
            StickyKeyState::Active {
                phase: SkPhase::Latched,
                ..
            } if !quick_release && !event.pressed => {
                self.sticky_key_state = StickyKeyState::None;
                true
            }
            _ => false,
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

        // If the SK is still physically held (Pressed phase), the deadline fired but the
        // key hasn't been released yet. Don't clear the latch — the physical release
        // handler (process_sticky_*) will transition Held→None cleanly. For pure-mod,
        // the deadline was set on press (→ Held on any other key press), so this can
        // only happen when the key is held and idle. For layer and tap-key shapes, the
        // deadline fires in the same scenario.
        // Clear the deadline to avoid busy-looping on every iteration.
        if matches!(self.sticky_key_state, StickyKeyState::Active { pressed: true, .. }) {
            debug!(
                "StickyKey timeout fired while key is still held — clearing deadline, deferring to physical release"
            );
            if let StickyKeyState::Active { deadline, .. } = &mut self.sticky_key_state {
                *deadline = None;
            }
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
            let activate_on_keypress = self.keymap.sticky_key_config().activate_on_keypress;
            matches!(
                self.sticky_key_state,
                StickyKeyState::Active {
                    phase: SkPhase::Held,
                    ..
                }
            ) || activate_on_keypress
        } else {
            // tap-key shape always reports; layer shape never does (deactivating emits nothing).
            !self.sticky_key_state.is_layer()
        };

        // A tap-key may still have its HID key registered when it is displaced by a different
        // StickyKey while physically held. Unregister it before clearing the latch so it cannot
        // remain stuck in the report.
        if let StickyKeyState::Active {
            key: KeyCode::Hid(hid_key),
            layer: None,
            source,
            ..
        } = self.sticky_key_state
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
        if let StickyKeyState::Active {
            layer: Some(layer_num), ..
        } = self.sticky_key_state
        {
            self.keymap.deactivate_layer(layer_num);
        }

        self.sticky_key_state = StickyKeyState::None;
        if needs_report {
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }
}
