//! Sticky modifier, layer, and tap-key behavior.
//!
//! The three effects share one latch lifecycle, but modifier and layer state
//! remain independent. Tap keys are deliberately exclusive because they keep
//! both a HID key and modifiers live between repetitions.

use embassy_time::{Duration, Instant};
use rmk_types::action::Action;
use rmk_types::keycode::HidKeyCode;
use rmk_types::modifier::ModifierCombination;

use crate::config::StickyKeyReleaseMode;
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
    /// The configured timeout elapsed while a producer remained down.
    TimedOut,
}

#[derive(Clone, Copy, Debug)]
struct Latch<T> {
    value: T,
    source: KeyboardEventPos,
    policy: StickyKeyPolicy,
    phase: LatchPhase,
    repeat_count: u16,
    deadline: Option<Instant>,
    buffered_claim: bool,
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
            buffered_claim: false,
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
        self.buffered_claim = false;
    }

    fn on_physical_release(&mut self, owner: Option<KeyboardEventPos>, now: Instant) -> PhysicalRelease {
        if owner.is_some_and(|owner| owner != self.source) {
            return PhysicalRelease::Ignored;
        }
        match self.phase {
            LatchPhase::Pressed => {
                if self.policy.release_on_keyup_after_timeout && self.deadline.is_some_and(|deadline| deadline <= now) {
                    self.phase = LatchPhase::TimedOut;
                    self.deadline = None;
                    return PhysicalRelease::Released;
                }
                self.phase = LatchPhase::Latched;
                self.deadline = deadline_from_timeout(self.policy.timeout);
                PhysicalRelease::Latched
            }
            LatchPhase::Held => PhysicalRelease::Released,
            LatchPhase::TimedOut => PhysicalRelease::Released,
            LatchPhase::Latched => PhysicalRelease::Ignored,
        }
    }

    fn mark_foreign_key(&mut self) {
        if matches!(self.phase, LatchPhase::Pressed | LatchPhase::TimedOut) {
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

    fn claim_buffered_press(&mut self, source: KeyboardEventPos) {
        if self.phase == LatchPhase::Latched && self.source != source && self.trigger_for_key(true) {
            self.buffered_claim = true;
            self.deadline = None;
        }
    }

    fn finish_buffered_claim(&mut self) {
        if self.buffered_claim {
            self.buffered_claim = false;
            self.deadline = deadline_from_timeout(self.policy.timeout);
        }
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
            if self.policy.release_on_keyup_after_timeout {
                self.phase = LatchPhase::TimedOut;
            }
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

const MAX_STICKY_MODIFIER_PRODUCERS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModifierProducer {
    source: KeyboardEventPos,
    modifiers: ModifierCombination,
}

/// Physical ownership and host-report state for accumulated sticky modifiers.
///
/// Positions distinguish a canceled producer's late release from a newer
/// latch. The modifier fallback supports combo outputs, whose release event can
/// come from a different constituent position than their press event.
#[derive(Clone, Copy, Debug)]
struct StickyModifierEffect {
    modifiers: ModifierCombination,
    producers: [Option<ModifierProducer>; MAX_STICKY_MODIFIER_PRODUCERS],
    host_visible: bool,
}

impl StickyModifierEffect {
    fn new(modifiers: ModifierCombination, source: KeyboardEventPos) -> Self {
        let mut effect = Self {
            modifiers,
            producers: [None; MAX_STICKY_MODIFIER_PRODUCERS],
            host_visible: false,
        };
        effect.begin_press(modifiers, source);
        effect
    }

    fn begin_press(&mut self, modifiers: ModifierCombination, source: KeyboardEventPos) {
        self.modifiers |= modifiers;
        let producer = ModifierProducer { source, modifiers };
        if let Some(slot) = self.producers.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(producer);
        } else {
            warn!(
                "Too many simultaneous Sticky modifier producers; ignoring {:?}",
                producer
            );
        }
    }

    fn release_at(&mut self, index: usize) -> bool {
        self.producers[index] = None;
        self.producers.iter().all(Option::is_none)
    }

    fn on_exact_release(&mut self, modifiers: ModifierCombination, source: KeyboardEventPos) -> Option<bool> {
        let exact = ModifierProducer { source, modifiers };
        self.producers
            .iter()
            .position(|producer| *producer == Some(exact))
            .map(|index| self.release_at(index))
    }

    fn on_combo_release(&mut self, modifiers: ModifierCombination) -> Option<bool> {
        self.producers
            .iter()
            .position(|producer| producer.is_some_and(|producer| producer.modifiers == modifiers))
            .map(|index| self.release_at(index))
    }
}

/// Layer state owned by a Sticky Key lifecycle.
#[derive(Clone, Copy, Debug)]
struct StickyLayerEffect {
    layer: u8,
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
    canceled_modifier_releases: [Option<ModifierProducer>; MAX_STICKY_MODIFIER_PRODUCERS],
    layer: Option<Latch<StickyLayerEffect>>,
    tap_key: Option<Latch<TapKeyEffect>>,
}

impl StickyKeyState {
    fn remember_canceled_modifier_release(&mut self, producer: ModifierProducer) {
        if let Some(slot) = self.canceled_modifier_releases.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(producer);
        } else {
            warn!("Too many canceled Sticky modifier releases; dropping {:?}", producer);
        }
    }

    fn consume_exact_canceled_modifier_release(
        &mut self,
        modifiers: ModifierCombination,
        source: KeyboardEventPos,
    ) -> bool {
        let exact = ModifierProducer { source, modifiers };
        let Some(index) = self
            .canceled_modifier_releases
            .iter()
            .position(|producer| *producer == Some(exact))
        else {
            return false;
        };
        self.canceled_modifier_releases[index] = None;
        true
    }

    fn consume_canceled_combo_release(&mut self, modifiers: ModifierCombination) -> bool {
        let Some(index) = self
            .canceled_modifier_releases
            .iter()
            .position(|producer| producer.is_some_and(|producer| producer.modifiers == modifiers))
        else {
            return false;
        };
        self.canceled_modifier_releases[index] = None;
        true
    }

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

    pub(crate) fn claim_buffered_press(&mut self, event: KeyboardEvent) {
        if !event.pressed {
            return;
        }
        if let Some(modifier) = &mut self.modifier {
            modifier.claim_buffered_press(event.pos);
        }
        if let Some(layer) = &mut self.layer {
            layer.claim_buffered_press(event.pos);
        }
        if let Some(tap_key) = &mut self.tap_key {
            tap_key.claim_buffered_press(event.pos);
        }
    }

    pub(crate) fn finish_buffered_claim(&mut self) {
        if let Some(modifier) = &mut self.modifier {
            modifier.finish_buffered_claim();
        }
        if let Some(layer) = &mut self.layer {
            layer.finish_buffered_claim();
        }
        if let Some(tap_key) = &mut self.tap_key {
            tap_key.finish_buffered_claim();
        }
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

    pub(crate) fn mark_modifier_host_visible(&mut self) {
        if let Some(modifier) = &mut self.modifier {
            modifier.value.host_visible = true;
        }
    }
}

impl Keyboard<'_> {
    pub(crate) async fn process_action_sticky_key(&mut self, action: Action, profile: u8, event: KeyboardEvent) {
        match action {
            Action::Modifier(modifiers) => self.process_sticky_modifier(modifiers, profile, event).await,
            Action::LayerOn(layer) => self.process_sticky_layer(layer, profile, event).await,
            Action::KeyWithModifier(key, modifiers) => {
                self.process_sticky_tap_key(key, modifiers, profile, event).await
            }
            _ => warn!("Unsupported Sticky Key action: {:?}", action),
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
                self.sticky_key_state
                    .remember_canceled_modifier_release(ModifierProducer {
                        source: event.pos,
                        modifiers,
                    });
                self.release_sticky_modifier().await;
                return;
            }

            match &mut self.sticky_key_state.modifier {
                Some(latch) => {
                    latch.value.begin_press(modifiers, event.pos);
                    latch.begin_press(event.pos, policy);
                }
                None => {
                    self.sticky_key_state.modifier = Some(Latch::new(
                        StickyModifierEffect::new(modifiers, event.pos),
                        event.pos,
                        policy,
                    ));
                }
            }
            if policy.activate_on_keypress {
                self.sticky_key_state.mark_modifier_host_visible();
                self.send_keyboard_report_with_resolved_modifiers(true).await;
            }
        } else {
            if self
                .sticky_key_state
                .consume_exact_canceled_modifier_release(modifiers, event.pos)
            {
                return;
            }
            if let Some(latch) = &mut self.sticky_key_state.modifier
                && let Some(last) = latch.value.on_exact_release(modifiers, event.pos)
            {
                if last && latch.on_physical_release(None, Instant::now()) == PhysicalRelease::Released {
                    self.release_sticky_modifier().await;
                }
                return;
            }
            if self.sticky_key_state.consume_canceled_combo_release(modifiers) {
                return;
            }
            if let Some(latch) = &mut self.sticky_key_state.modifier
                && latch.value.on_combo_release(modifiers).is_some_and(|last| last)
                && latch.on_physical_release(None, Instant::now()) == PhysicalRelease::Released
            {
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
                    self.keymap.activate_layer(layer);
                    previous.begin_press(event.pos, policy);
                    self.sticky_key_state.layer = Some(previous);
                    return;
                }
                self.release_sticky_layer_effect(previous.value);
            }
            self.keymap.activate_layer(layer);
            self.sticky_key_state.layer = Some(Latch::new(StickyLayerEffect { layer }, event.pos, policy));
        } else if let Some(latch) = &mut self.sticky_key_state.layer
            && latch.on_physical_release(Some(event.pos), Instant::now()) == PhysicalRelease::Released
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
                self.process_action_key(key, event).await;
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
                .on_physical_release(Some(event.pos), Instant::now());
            self.process_action_key(key, event).await;
        }
    }

    /// Apply a foreign key event to the independently active modifier and
    /// layer latches.
    pub(crate) fn update_sticky_key(&mut self, event: KeyboardEvent) -> StickyKeyUpdate {
        let mut update = StickyKeyUpdate::default();

        if let Some(modifier) = &mut self.sticky_key_state.modifier {
            match modifier.phase {
                LatchPhase::Pressed | LatchPhase::TimedOut => modifier.mark_foreign_key(),
                LatchPhase::Latched if modifier.trigger_for_key(event.pressed) => {
                    update.modifier_was_host_visible = modifier.value.host_visible;
                    self.sticky_key_state.modifier = None;
                    update.modifier_consumed = true;
                }
                LatchPhase::Latched | LatchPhase::Held => {}
            }
        }

        if let Some(layer) = &mut self.sticky_key_state.layer {
            match layer.phase {
                LatchPhase::Pressed | LatchPhase::TimedOut => layer.mark_foreign_key(),
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

    pub(crate) async fn release_sticky_key_on_layer_event(&mut self, event: StickyKeyReleaseMode) {
        if self
            .sticky_key_state
            .modifier
            .is_some_and(|latch| latch.policy.release_mode.intersects(event))
        {
            self.release_sticky_modifier().await;
        }
        if self
            .sticky_key_state
            .layer
            .is_some_and(|latch| latch.policy.release_mode.intersects(event))
        {
            self.release_sticky_layer();
        }
        if self
            .sticky_key_state
            .tap_key
            .is_some_and(|latch| latch.policy.release_mode.intersects(event))
        {
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
        for producer in modifier.value.producers.into_iter().flatten() {
            self.sticky_key_state.remember_canceled_modifier_release(producer);
        }
        if modifier.value.host_visible {
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
    }

    fn release_sticky_layer(&mut self) {
        if let Some(layer) = self.sticky_key_state.layer.take() {
            self.release_sticky_layer_effect(layer.value);
        }
    }

    fn release_sticky_layer_effect(&self, effect: StickyLayerEffect) {
        self.keymap.deactivate_layer_if_active(effect.layer);
    }

    pub(crate) async fn release_tap_key(&mut self) {
        let Some(tap_key) = self.sticky_key_state.tap_key.take() else {
            return;
        };
        if tap_key.phase == LatchPhase::Pressed {
            self.process_action_key(
                tap_key.value.key,
                KeyboardEvent {
                    pressed: false,
                    pos: tap_key.source,
                },
            )
            .await;
        } else {
            self.send_keyboard_report_with_resolved_modifiers(false).await;
        }
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
            release_on_keyup_after_timeout: false,
            max_repeat: 0,
            release_mode,
        }
    }

    #[test]
    fn modifier_effect_counts_overlapping_physical_producers() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL, pos(0)),
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
        );
        latch.value.begin_press(ModifierCombination::LSHIFT, pos(1));
        latch.begin_press(pos(1), policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE));

        assert_eq!(
            latch.value.on_exact_release(ModifierCombination::LCTRL, pos(0)),
            Some(false)
        );
        assert_eq!(latch.phase, LatchPhase::Pressed);
        assert_eq!(
            latch.value.on_exact_release(ModifierCombination::LSHIFT, pos(1)),
            Some(true)
        );
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Latched
        );
        assert_eq!(latch.phase, LatchPhase::Latched);
        assert_eq!(
            latch.value.modifiers,
            ModifierCombination::LCTRL | ModifierCombination::LSHIFT
        );
    }

    #[test]
    fn held_latch_releases_after_last_physical_producer() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL, pos(0)),
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
        );
        latch.value.begin_press(ModifierCombination::LSHIFT, pos(1));
        latch.begin_press(pos(1), policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE));
        latch.mark_foreign_key();

        assert_eq!(
            latch.value.on_exact_release(ModifierCombination::LCTRL, pos(0)),
            Some(false)
        );
        assert_eq!(
            latch.value.on_exact_release(ModifierCombination::LSHIFT, pos(1)),
            Some(true)
        );
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Released
        );
    }

    #[test]
    fn timeout_is_deferred_while_physical_producer_is_down() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL, pos(0)),
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
        );
        let deadline = latch.deadline.unwrap();

        assert_eq!(latch.timeout_disposition(deadline), TimeoutDisposition::Deferred);
        assert_eq!(latch.phase, LatchPhase::Pressed);
        assert_eq!(latch.deadline, None);
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Latched
        );
        assert_eq!(latch.phase, LatchPhase::Latched);
        assert!(latch.deadline.is_some());
    }

    #[test]
    fn configured_timeout_releases_on_physical_keyup() {
        let mut timeout_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        timeout_policy.release_on_keyup_after_timeout = true;
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL, pos(0)),
            pos(0),
            timeout_policy,
        );
        let deadline = latch.deadline.unwrap();

        assert_eq!(latch.timeout_disposition(deadline), TimeoutDisposition::Deferred);
        assert_eq!(latch.phase, LatchPhase::TimedOut);
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Released
        );
    }

    #[test]
    fn configured_release_checks_expired_deadline_before_timeout_poll() {
        let mut timeout_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        timeout_policy.release_on_keyup_after_timeout = true;
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL, pos(0)),
            pos(0),
            timeout_policy,
        );
        let deadline = latch.deadline.unwrap();

        assert_eq!(latch.on_physical_release(None, deadline), PhysicalRelease::Released);
        assert_eq!(latch.phase, LatchPhase::TimedOut);
        assert_eq!(latch.deadline, None);
    }

    #[test]
    fn buffered_foreign_press_claims_latch_until_action_resolves() {
        let mut latch = Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL, pos(0)),
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_PRESS),
        );
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Latched
        );

        latch.claim_buffered_press(pos(1));
        assert!(latch.buffered_claim);
        assert_eq!(latch.deadline, None);

        latch.finish_buffered_claim();
        assert!(!latch.buffered_claim);
        assert!(latch.deadline.is_some());
    }

    #[test]
    fn canceled_release_does_not_consume_a_new_modifier_latch() {
        let mut state = StickyKeyState::default();
        state.remember_canceled_modifier_release(ModifierProducer {
            source: pos(0),
            modifiers: ModifierCombination::LSHIFT,
        });
        state.modifier = Some(Latch::new(
            StickyModifierEffect::new(ModifierCombination::LCTRL, pos(1)),
            pos(1),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
        ));

        assert!(state.consume_exact_canceled_modifier_release(ModifierCombination::LSHIFT, pos(0)));
        assert!(state.modifier.is_some());
        assert!(!state.consume_exact_canceled_modifier_release(ModifierCombination::LCTRL, pos(1)));
    }

    #[test]
    fn new_exact_release_wins_over_same_modifier_combo_fallback() {
        let mut state = StickyKeyState::default();
        state.remember_canceled_modifier_release(ModifierProducer {
            source: pos(0),
            modifiers: ModifierCombination::LSHIFT,
        });
        let mut effect = StickyModifierEffect::new(ModifierCombination::LSHIFT, pos(1));

        assert_eq!(effect.on_exact_release(ModifierCombination::LSHIFT, pos(1)), Some(true));
        assert!(state.consume_exact_canceled_modifier_release(ModifierCombination::LSHIFT, pos(0)));
    }
}
