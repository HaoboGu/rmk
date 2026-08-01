//! Sticky modifier, layer, and tap-key behavior.
//!
//! The three effects share one latch lifecycle, but modifier and layer state
//! remain independent. Tap keys are deliberately exclusive because they keep
//! both a HID key and modifiers live between repetitions.

use embassy_time::{Duration, Instant};
use rmk_types::action::{StickyKeyAction, StickyKeyEffect};
use rmk_types::keycode::HidKeyCode;
use rmk_types::modifier::ModifierCombination;

use crate::config::StickyKeyReleaseMode;
use crate::event::state::LayerTransitionGeneration;
use crate::event::{KeyboardEvent, KeyboardEventPos};
use crate::keyboard::Keyboard;
use crate::keymap::{StickyKeyPolicy, StickyKeyShape};

fn deadline_from_timeout(timeout: Duration) -> Option<Instant> {
    (timeout != Duration::MAX).then(|| Instant::now() + timeout)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LatchPhase {
    /// One or more physical producers are still down.
    Pressed,
    /// Every producer is up and the effect is armed.
    Latched,
    /// A foreign key was pressed while a producer remained down.
    Held,
}

#[derive(Clone, Copy, Debug)]
struct Latch<T> {
    value: T,
    source: KeyboardEventPos,
    policy: StickyKeyPolicy,
    phase: LatchPhase,
    repeat_count: u16,
    deadline: Option<Instant>,
    layer_generation: LayerTransitionGeneration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalRelease {
    Ignored,
    Latched,
    Released,
}

impl<T> Latch<T> {
    fn new(value: T, source: KeyboardEventPos, policy: StickyKeyPolicy) -> Self {
        Self {
            value,
            source,
            policy,
            phase: LatchPhase::Pressed,
            repeat_count: 1,
            deadline: deadline_from_timeout(policy.timeout),
            layer_generation: LayerTransitionGeneration::current(),
        }
    }

    fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    fn begin_press(&mut self, source: KeyboardEventPos, policy: StickyKeyPolicy) {
        self.source = source;
        self.policy = policy;
        self.phase = LatchPhase::Pressed;
        self.deadline = deadline_from_timeout(policy.timeout);
        self.layer_generation = LayerTransitionGeneration::current();
    }

    fn on_physical_release(&mut self, owner: Option<KeyboardEventPos>) -> PhysicalRelease {
        if owner.is_some_and(|owner| owner != self.source) {
            return PhysicalRelease::Ignored;
        }
        match self.phase {
            LatchPhase::Pressed => {
                self.phase = LatchPhase::Latched;
                self.deadline = deadline_from_timeout(self.policy.timeout);
                PhysicalRelease::Latched
            }
            LatchPhase::Held => PhysicalRelease::Released,
            LatchPhase::Latched => PhysicalRelease::Ignored,
        }
    }

    fn mark_foreign_key(&mut self) {
        if self.phase == LatchPhase::Pressed {
            self.phase = LatchPhase::Held;
            self.deadline = None;
        }
    }

    fn trigger_for_key(&self, pressed: bool) -> bool {
        let trigger = if pressed {
            StickyKeyReleaseMode::OTHER_KEY_PRESS
        } else {
            StickyKeyReleaseMode::OTHER_KEY_RELEASE
        };
        self.policy.release_mode.intersects(trigger)
    }

    fn is_double_tap(&self, source: KeyboardEventPos, policy: StickyKeyPolicy) -> bool {
        self.phase == LatchPhase::Latched
            && self.source == source
            && policy.release_mode.intersects(StickyKeyReleaseMode::DOUBLE_TAP)
    }

    /// A timeout cannot erase a latch whose physical producer is still down:
    /// its later release must still complete the lifecycle.
    fn timeout_disposition(&mut self, now: Instant) -> TimeoutDisposition {
        if !self.deadline.is_some_and(|deadline| deadline <= now) {
            return TimeoutDisposition::Pending;
        }
        if self.phase == LatchPhase::Pressed {
            self.deadline = None;
            TimeoutDisposition::Deferred
        } else {
            TimeoutDisposition::Release
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimeoutDisposition {
    Pending,
    Deferred,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TapKeyEffect {
    key: HidKeyCode,
    modifiers: ModifierCombination,
}

/// Counted physical ownership used only by accumulated sticky modifiers.
/// Combo outputs may release from a different constituent position, so they
/// cannot use the source-specific ownership used by layer and tap-key effects.
#[derive(Clone, Copy, Debug)]
struct StickyModifierEffect {
    modifiers: ModifierCombination,
    pressed_count: u8,
}

impl StickyModifierEffect {
    fn new(modifiers: ModifierCombination) -> Self {
        Self {
            modifiers,
            pressed_count: 1,
        }
    }

    fn begin_press(&mut self, modifiers: ModifierCombination) {
        self.modifiers |= modifiers;
        self.pressed_count = self.pressed_count.saturating_add(1).max(1);
    }

    fn on_physical_release(&mut self) -> bool {
        if self.pressed_count == 0 {
            return false;
        }
        self.pressed_count -= 1;
        self.pressed_count == 0
    }
}

/// Layer state owned by a Sticky Key lifecycle.
#[derive(Clone, Copy, Debug)]
struct StickyLayerEffect {
    layer: u8,
    ownership_generation: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StickyKeyUpdate {
    pub(crate) modifier_consumed: bool,
    pub(crate) modifier_was_host_visible: bool,
}

/// Runtime composition for sticky effects.
///
/// Modifier and layer effects can coexist and therefore own distinct policies,
/// phases, sources, and deadlines. A tap key is exclusive with both.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StickyKeyState {
    modifier: Option<Latch<StickyModifierEffect>>,
    layer: Option<Latch<StickyLayerEffect>>,
    tap_key: Option<Latch<TapKeyEffect>>,
}

impl StickyKeyState {
    pub(crate) fn deadline(&self) -> Option<Instant> {
        [
            self.modifier.as_ref().and_then(Latch::deadline),
            self.layer.as_ref().and_then(Latch::deadline),
            self.tap_key.as_ref().and_then(Latch::deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(crate) fn has_tap_key(&self) -> bool {
        self.tap_key.is_some()
    }

    pub(crate) fn tap_key_releases_on(&self, pressed: bool) -> bool {
        self.tap_key
            .as_ref()
            .is_some_and(|tap_key| tap_key.trigger_for_key(pressed))
    }

    pub(crate) fn modifier_releases_on_press(&self) -> bool {
        self.modifier
            .as_ref()
            .is_some_and(|modifier| modifier.trigger_for_key(true))
    }

    pub(crate) fn modifiers(&self, pressed: bool) -> ModifierCombination {
        let mut modifiers = ModifierCombination::new();
        if let Some(modifier) = self.modifier
            && (pressed || modifier.phase == LatchPhase::Held)
        {
            modifiers |= modifier.value.modifiers;
        }
        if let Some(tap_key) = self.tap_key {
            modifiers |= tap_key.value.modifiers;
        }
        modifiers
    }
}

impl Keyboard<'_> {
    pub(crate) async fn process_action_sticky_key(&mut self, action: StickyKeyAction, event: KeyboardEvent) {
        match action.effect {
            StickyKeyEffect::Modifier(modifiers) => {
                self.process_sticky_modifier(modifiers, action.profile, event).await
            }
            StickyKeyEffect::Layer(layer) => self.process_sticky_layer(layer, action.profile, event).await,
            StickyKeyEffect::TapKey { key, modifiers } => {
                self.process_sticky_tap_key(key, modifiers, action.profile, event).await
            }
        }
    }

    async fn process_sticky_modifier(
        &mut self,
        modifiers: ModifierCombination,
        profile_index: u8,
        event: KeyboardEvent,
    ) {
        let policy = self.keymap.sticky_key_profile(profile_index, StickyKeyShape::PureMod);

        if event.pressed {
            self.release_tap_key().await;
            if self
                .sticky_key_state
                .modifier
                .is_some_and(|latch| latch.is_double_tap(event.pos, policy))
            {
                self.release_sticky_modifier().await;
                return;
            }

            match &mut self.sticky_key_state.modifier {
                Some(latch) => {
                    latch.value.begin_press(modifiers);
                    latch.begin_press(event.pos, policy);
                }
                None => {
                    self.sticky_key_state.modifier =
                        Some(Latch::new(StickyModifierEffect::new(modifiers), event.pos, policy));
                }
            }
            if policy.activate_on_keypress {
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else if let Some(latch) = &mut self.sticky_key_state.modifier {
            // Combo outputs may be released by a different constituent
            // position, so modifier producers use counted ownership.
            if latch.value.on_physical_release() && latch.on_physical_release(None) == PhysicalRelease::Released {
                self.release_sticky_modifier().await;
            }
        }
    }

    async fn process_sticky_layer(&mut self, layer: u8, profile_index: u8, event: KeyboardEvent) {
        let policy = self.keymap.sticky_key_profile(profile_index, StickyKeyShape::Layer);

        if event.pressed {
            self.release_tap_key().await;
            if layer as usize >= self.keymap.num_layer() {
                // Keep KeyMap's established diagnostic, but never arm an invalid layer.
                self.keymap.activate_layer(layer);
                return;
            }
            if self
                .sticky_key_state
                .layer
                .is_some_and(|latch| latch.is_double_tap(event.pos, policy))
            {
                self.release_sticky_layer();
                return;
            }
            if let Some(mut previous) = self.sticky_key_state.layer.take() {
                if previous.value.layer == layer {
                    previous.begin_press(event.pos, policy);
                    self.sticky_key_state.layer = Some(previous);
                    return;
                }
                self.release_sticky_layer_effect(previous.value);
            }
            let activated_by_us = self.keymap.activate_layer_if_inactive(layer);
            self.sticky_key_state.layer = Some(Latch::new(
                StickyLayerEffect {
                    layer,
                    ownership_generation: activated_by_us.then(|| self.keymap.layer_generation(layer)).flatten(),
                },
                event.pos,
                policy,
            ));
        } else if let Some(latch) = &mut self.sticky_key_state.layer
            && latch.on_physical_release(Some(event.pos)) == PhysicalRelease::Released
        {
            self.release_sticky_layer();
        }
    }

    async fn process_sticky_tap_key(
        &mut self,
        key: HidKeyCode,
        modifiers: ModifierCombination,
        profile_index: u8,
        event: KeyboardEvent,
    ) {
        let policy = self.keymap.sticky_key_profile(profile_index, StickyKeyShape::TapKey);

        if event.pressed {
            self.release_sticky_modifier().await;
            self.release_sticky_layer();

            let effect = TapKeyEffect { key, modifiers };
            let same_tap_key = self
                .sticky_key_state
                .tap_key
                .is_some_and(|latch| latch.source == event.pos && latch.value == effect);
            if same_tap_key
                && self
                    .sticky_key_state
                    .tap_key
                    .is_some_and(|latch| latch.is_double_tap(event.pos, policy))
            {
                self.release_tap_key().await;
                return;
            }
            if !same_tap_key {
                self.release_tap_key().await;
            }

            let mut deactivate = false;
            match &mut self.sticky_key_state.tap_key {
                Some(latch) => {
                    latch.repeat_count = latch.repeat_count.saturating_add(1);
                    if policy.max_repeat > 0 && latch.repeat_count > policy.max_repeat {
                        deactivate = true;
                    } else {
                        latch.begin_press(event.pos, policy);
                    }
                }
                None => {
                    self.sticky_key_state.tap_key = Some(Latch::new(effect, event.pos, policy));
                }
            }

            if deactivate {
                self.release_tap_key().await;
            } else {
                self.register_key(key, event);
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else if self
            .sticky_key_state
            .tap_key
            .is_some_and(|latch| latch.source == event.pos && latch.phase == LatchPhase::Pressed)
        {
            self.sticky_key_state
                .tap_key
                .as_mut()
                .expect("tap key checked above")
                .on_physical_release(Some(event.pos));
            self.unregister_key(key, event);
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }

    /// Apply a foreign key event to the independently active modifier and
    /// layer latches.
    pub(crate) fn update_sticky_key(&mut self, event: KeyboardEvent) -> StickyKeyUpdate {
        let mut update = StickyKeyUpdate::default();

        if let Some(modifier) = &mut self.sticky_key_state.modifier {
            match modifier.phase {
                LatchPhase::Pressed => modifier.mark_foreign_key(),
                LatchPhase::Latched if modifier.trigger_for_key(event.pressed) => {
                    update.modifier_was_host_visible = modifier.policy.activate_on_keypress;
                    self.sticky_key_state.modifier = None;
                    update.modifier_consumed = true;
                }
                LatchPhase::Latched | LatchPhase::Held => {}
            }
        }

        if let Some(layer) = &mut self.sticky_key_state.layer {
            match layer.phase {
                LatchPhase::Pressed => layer.mark_foreign_key(),
                LatchPhase::Latched if layer.trigger_for_key(event.pressed) => {
                    let layer = layer.value;
                    self.sticky_key_state.layer = None;
                    self.release_sticky_layer_effect(layer);
                }
                LatchPhase::Latched | LatchPhase::Held => {}
            }
        }

        update
    }

    pub(crate) async fn release_sticky_key_on_layer_event(
        &mut self,
        event: StickyKeyReleaseMode,
        generation: Option<LayerTransitionGeneration>,
    ) {
        if self.sticky_key_state.modifier.is_some_and(|latch| {
            generation.map_or_else(
                || latch.policy.release_mode.intersects(event),
                |generation| latch.policy.release_mode.intersects(event) && generation.is_after(latch.layer_generation),
            )
        }) {
            self.release_sticky_modifier().await;
        }
        if self.sticky_key_state.layer.is_some_and(|latch| {
            generation.map_or_else(
                || latch.policy.release_mode.intersects(event),
                |generation| latch.policy.release_mode.intersects(event) && generation.is_after(latch.layer_generation),
            )
        }) {
            self.release_sticky_layer();
        }
        if self.sticky_key_state.tap_key.is_some_and(|latch| {
            generation.map_or_else(
                || latch.policy.release_mode.intersects(event),
                |generation| latch.policy.release_mode.intersects(event) && generation.is_after(latch.layer_generation),
            )
        }) {
            self.release_tap_key().await;
        }
    }

    pub(crate) async fn release_sticky_key_if_active_on_timeout(&mut self) {
        let now = Instant::now();
        if self
            .sticky_key_state
            .modifier
            .as_mut()
            .is_some_and(|latch| latch.timeout_disposition(now) == TimeoutDisposition::Release)
        {
            self.release_sticky_modifier().await;
        }
        if self
            .sticky_key_state
            .layer
            .as_mut()
            .is_some_and(|latch| latch.timeout_disposition(now) == TimeoutDisposition::Release)
        {
            self.release_sticky_layer();
        }
        if self
            .sticky_key_state
            .tap_key
            .as_mut()
            .is_some_and(|latch| latch.timeout_disposition(now) == TimeoutDisposition::Release)
        {
            self.release_tap_key().await;
        }
    }

    async fn release_sticky_modifier(&mut self) {
        let Some(modifier) = self.sticky_key_state.modifier.take() else {
            return;
        };
        if modifier.phase == LatchPhase::Held || modifier.policy.activate_on_keypress {
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }

    fn release_sticky_layer(&mut self) {
        if let Some(layer) = self.sticky_key_state.layer.take() {
            self.release_sticky_layer_effect(layer.value);
        }
    }

    fn release_sticky_layer_effect(&self, effect: StickyLayerEffect) {
        if effect.ownership_generation.is_some()
            && effect.ownership_generation == self.keymap.layer_generation(effect.layer)
        {
            self.keymap.deactivate_layer_if_active(effect.layer);
        }
    }

    pub(crate) async fn release_tap_key(&mut self) {
        let Some(tap_key) = self.sticky_key_state.tap_key.take() else {
            return;
        };
        if tap_key.phase == LatchPhase::Pressed {
            self.unregister_key(
                tap_key.value.key,
                KeyboardEvent {
                    pressed: false,
                    pos: tap_key.source,
                },
            );
        }
        self.send_keyboard_report_with_resolved_modifiers(false).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(col: u8) -> KeyboardEventPos {
        KeyboardEventPos::key_pos(col, 0)
    }

    fn policy(release_mode: StickyKeyReleaseMode) -> StickyKeyPolicy {
        StickyKeyPolicy {
            timeout: Duration::from_secs(1),
            activate_on_keypress: false,
            max_repeat: 0,
            release_mode,
        }
    }

    #[test]
    fn modifier_effect_counts_overlapping_physical_producers() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL),
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
        );
        latch.value.begin_press(ModifierCombination::LSHIFT);
        latch.begin_press(pos(1), policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE));

        assert!(!latch.value.on_physical_release());
        assert_eq!(latch.phase, LatchPhase::Pressed);
        assert!(latch.value.on_physical_release());
        assert_eq!(latch.on_physical_release(None), PhysicalRelease::Latched);
        assert_eq!(latch.phase, LatchPhase::Latched);
        assert_eq!(
            latch.value.modifiers,
            ModifierCombination::LCTRL | ModifierCombination::LSHIFT
        );
    }

    #[test]
    fn held_latch_releases_after_last_physical_producer() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL),
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
        );
        latch.value.begin_press(ModifierCombination::LSHIFT);
        latch.begin_press(pos(1), policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE));
        latch.mark_foreign_key();

        assert!(!latch.value.on_physical_release());
        assert!(latch.value.on_physical_release());
        assert_eq!(latch.on_physical_release(None), PhysicalRelease::Released);
    }

    #[test]
    fn timeout_is_deferred_while_physical_producer_is_down() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL),
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
        );
        let deadline = latch.deadline.unwrap();

        assert_eq!(latch.timeout_disposition(deadline), TimeoutDisposition::Deferred);
        assert_eq!(latch.phase, LatchPhase::Pressed);
        assert_eq!(latch.deadline, None);
    }

    #[test]
    fn sticky_layer_ownership_generation_detects_later_mutations() {
        let effect = StickyLayerEffect {
            layer: 2,
            ownership_generation: Some(7),
        };
        assert_eq!(effect.ownership_generation, Some(7));
    }

    #[test]
    fn preexisting_sticky_layer_never_claims_cleanup_ownership() {
        let effect = StickyLayerEffect {
            layer: 2,
            ownership_generation: None,
        };
        assert_eq!(effect.ownership_generation, None);
    }

    #[test]
    fn external_layer_events_only_release_their_current_lifecycle() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL),
            pos(0),
            policy(StickyKeyReleaseMode::LAYER_ENTER),
        );
        latch.layer_generation = LayerTransitionGeneration::from_raw(10);

        assert!(!LayerTransitionGeneration::from_raw(10).is_after(latch.layer_generation));
        assert!(LayerTransitionGeneration::from_raw(11).is_after(latch.layer_generation));
    }
}
