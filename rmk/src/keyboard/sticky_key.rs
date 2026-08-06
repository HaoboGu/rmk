//! Sticky Key behavior for held [`Action`]s.
//!
//! Every active action uses the same entry lifecycle. Action-specific code is
//! confined to applying and releasing the effect; modifier and layer entries
//! may coexist, while a modified tap key remains deliberately exclusive.

use embassy_time::{Duration, Instant};
use rmk_types::action::Action;
use rmk_types::keycode::{HidKeyCode, KeyCode};
use rmk_types::modifier::ModifierCombination;

#[cfg(test)]
use crate::config::StickyKeyHoldDuration;
use crate::config::StickyKeyReleaseMode;
use crate::event::{KeyboardEvent, KeyboardEventPos};
use crate::keyboard::Keyboard;
use crate::keymap::{StickyKeyPolicy, StickyKeyShape};

fn deadline_from(start: Instant, duration: Duration) -> Option<Instant> {
    (duration != Duration::MAX).then(|| start + duration)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StickyPhase {
    /// One or more physical producers are still down.
    Pressed,
    /// A producer remains down, but its press deadline is absent or consumed.
    /// `StickyEntry::timing_marker` stores the continuous chord's start without scheduling a wake.
    PressDeadlineInactive,
    /// Every producer is up and the effect is armed.
    Latched,
    /// A foreign key was pressed while a producer remained down.
    Held,
    /// The configured key-up release threshold elapsed while a producer remained down.
    HoldQualified,
}

#[derive(Clone, Copy, Debug)]
struct StickyEntry {
    /// Canonical identity of the held action. Effect code derives its shape
    /// from this value instead of parallel modifier/layer/tap-key slots.
    action: Action,
    source: KeyboardEventPos,
    policy: StickyKeyPolicy,
    phase: StickyPhase,
    repeat_count: u16,
    /// Active deadline in `Pressed`/`Latched`, or the chord start while a
    /// physical producer is down and no wakeup is needed.
    timing_marker: Option<Instant>,
    buffered_claim: bool,
    /// Action-local state that cannot be derived from the held `Action`.
    effect_state: StickyEffectState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalRelease {
    Ignored,
    Latched,
    Released,
}

impl StickyEntry {
    fn new(action: Action, source: KeyboardEventPos, policy: StickyKeyPolicy, pressed_at: Instant) -> Self {
        let (phase, timing_marker) = Self::press_state(policy, pressed_at, pressed_at);
        Self {
            action,
            source,
            policy,
            phase,
            repeat_count: 1,
            timing_marker,
            buffered_claim: false,
            effect_state: match action {
                Action::Modifier(modifiers) => {
                    StickyEffectState::Modifier(StickyModifierEffect::new(modifiers, source))
                }
                _ => StickyEffectState::ActionOnly,
            },
        }
    }

    fn is_modifier(&self) -> bool {
        matches!(self.action, Action::Modifier(_))
    }

    fn is_layer(&self) -> bool {
        matches!(self.action, Action::LayerOn(_))
    }

    fn is_tap_key(&self) -> bool {
        matches!(self.action, Action::KeyWithModifier(_, _))
    }

    fn modifiers(&self) -> ModifierCombination {
        match self.action {
            Action::Modifier(modifiers) | Action::KeyWithModifier(_, modifiers) => modifiers,
            _ => ModifierCombination::new(),
        }
    }

    fn layer(&self) -> Option<u8> {
        match self.action {
            Action::LayerOn(layer) => Some(layer),
            _ => None,
        }
    }

    fn add_modifier_producer(&mut self, modifiers: ModifierCombination, source: KeyboardEventPos) {
        self.action = Action::Modifier(self.modifiers() | modifiers);
        self.modifier_effect_mut().begin_press(modifiers, source);
    }

    fn on_exact_modifier_release(&mut self, modifiers: ModifierCombination, source: KeyboardEventPos) -> Option<bool> {
        self.modifier_effect_mut().on_exact_release(modifiers, source)
    }

    fn on_combo_modifier_release(&mut self, modifiers: ModifierCombination) -> Option<bool> {
        self.modifier_effect_mut().on_combo_release(modifiers)
    }

    fn modifier_effect(&self) -> &StickyModifierEffect {
        match &self.effect_state {
            StickyEffectState::Modifier(effect) => effect,
            StickyEffectState::ActionOnly => unreachable!("modifier action must own modifier effect state"),
        }
    }

    fn modifier_effect_mut(&mut self) -> &mut StickyModifierEffect {
        match &mut self.effect_state {
            StickyEffectState::Modifier(effect) => effect,
            StickyEffectState::ActionOnly => unreachable!("modifier action must own modifier effect state"),
        }
    }

    fn deadline(&self) -> Option<Instant> {
        if matches!(self.phase, StickyPhase::Pressed | StickyPhase::Latched) {
            self.timing_marker
        } else {
            None
        }
    }

    fn press_state(policy: StickyKeyPolicy, chord_started_at: Instant, now: Instant) -> (StickyPhase, Option<Instant>) {
        let Some(hold_duration) = policy.release_after_hold.duration() else {
            return (StickyPhase::PressDeadlineInactive, Some(chord_started_at));
        };

        match deadline_from(chord_started_at, hold_duration) {
            Some(deadline) if deadline > now => (StickyPhase::Pressed, Some(deadline)),
            Some(_) => (StickyPhase::HoldQualified, Some(chord_started_at)),
            None => (StickyPhase::PressDeadlineInactive, Some(chord_started_at)),
        }
    }

    fn begin_press(&mut self, source: KeyboardEventPos, policy: StickyKeyPolicy, pressed_at: Instant) {
        self.source = source;
        self.policy = policy;
        (self.phase, self.timing_marker) = Self::press_state(policy, pressed_at, pressed_at);
        self.buffered_claim = false;
    }

    /// Add a producer to the current modifier chord without forgetting how
    /// long the chord has already been held. The latest producer still selects
    /// the policy, but its threshold is measured from the chord's first press.
    fn begin_modifier_press(&mut self, source: KeyboardEventPos, policy: StickyKeyPolicy, pressed_at: Instant) {
        let chord_started_at = match self.phase {
            StickyPhase::Pressed => self
                .timing_marker
                .zip(self.policy.release_after_hold.duration())
                .map(|(deadline, hold_duration)| deadline - hold_duration),
            StickyPhase::PressDeadlineInactive | StickyPhase::HoldQualified => self.timing_marker,
            StickyPhase::Latched | StickyPhase::Held => None,
        }
        .unwrap_or(pressed_at);

        self.source = source;
        self.policy = policy;
        (self.phase, self.timing_marker) = Self::press_state(policy, chord_started_at, pressed_at);
        self.buffered_claim = false;
    }

    fn on_physical_release(&mut self, owner: Option<KeyboardEventPos>, now: Instant) -> PhysicalRelease {
        if owner.is_some_and(|owner| owner != self.source) {
            return PhysicalRelease::Ignored;
        }
        match self.phase {
            StickyPhase::Pressed => {
                if self.policy.release_after_hold.duration().is_some()
                    && self.timing_marker.is_some_and(|deadline| deadline <= now)
                {
                    self.phase = StickyPhase::HoldQualified;
                    self.timing_marker = None;
                    return PhysicalRelease::Released;
                }
                self.phase = StickyPhase::Latched;
                self.timing_marker = deadline_from(now, self.policy.timeout);
                PhysicalRelease::Latched
            }
            StickyPhase::PressDeadlineInactive => {
                self.phase = StickyPhase::Latched;
                self.timing_marker = deadline_from(now, self.policy.timeout);
                PhysicalRelease::Latched
            }
            StickyPhase::Held | StickyPhase::HoldQualified => {
                self.timing_marker = None;
                PhysicalRelease::Released
            }
            StickyPhase::Latched => PhysicalRelease::Ignored,
        }
    }

    fn mark_foreign_key(&mut self) {
        if matches!(
            self.phase,
            StickyPhase::Pressed | StickyPhase::PressDeadlineInactive | StickyPhase::HoldQualified
        ) {
            self.phase = StickyPhase::Held;
            self.timing_marker = None;
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
        if self.phase == StickyPhase::Latched && self.source != source && self.trigger_for_key(true) {
            self.buffered_claim = true;
            self.timing_marker = None;
        }
    }

    fn finish_buffered_claim(&mut self) {
        if self.buffered_claim {
            self.buffered_claim = false;
            self.timing_marker = deadline_from(Instant::now(), self.policy.timeout);
        }
    }

    fn is_double_tap(&self, source: KeyboardEventPos, policy: StickyKeyPolicy) -> bool {
        self.phase == StickyPhase::Latched
            && self.source == source
            && policy.release_mode.intersects(StickyKeyReleaseMode::DOUBLE_TAP)
    }

    /// A timeout cannot erase a latch whose physical producer is still down:
    /// its later release must still complete the lifecycle.
    fn deadline_disposition(&mut self, now: Instant) -> DeadlineDisposition {
        let Some(deadline) = self.deadline().filter(|deadline| *deadline <= now) else {
            return DeadlineDisposition::Pending;
        };
        if self.phase == StickyPhase::Pressed {
            self.timing_marker = self
                .policy
                .release_after_hold
                .duration()
                .map(|hold_duration| deadline - hold_duration);
            self.phase = StickyPhase::HoldQualified;
            DeadlineDisposition::Deferred
        } else {
            DeadlineDisposition::Release
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeadlineDisposition {
    Pending,
    Deferred,
    Release,
}

const MAX_STICKY_MODIFIER_PRODUCERS: usize = 8;
const MAX_ACTIVE_STICKY_KEYS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    producers: [Option<ModifierProducer>; MAX_STICKY_MODIFIER_PRODUCERS],
    host_visible: bool,
}

impl StickyModifierEffect {
    fn new(modifiers: ModifierCombination, source: KeyboardEventPos) -> Self {
        let mut effect = Self {
            producers: [None; MAX_STICKY_MODIFIER_PRODUCERS],
            host_visible: false,
        };
        effect.begin_press(modifiers, source);
        effect
    }

    fn begin_press(&mut self, modifiers: ModifierCombination, source: KeyboardEventPos) {
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

#[derive(Clone, Copy, Debug)]
enum StickyEffectState {
    /// Layer and tap-key effects are represented completely by `StickyEntry::action`.
    ActionOnly,
    /// Modifier effects additionally track physical producers and host visibility.
    Modifier(StickyModifierEffect),
}

#[derive(Clone, Copy, Debug, Default)]
struct StickyKeyUpdate {
    modifier_consumed: bool,
    modifier_was_host_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StickyKeyPostAction {
    None,
    Release,
}

/// Bounded runtime collection for held actions.
///
/// Two entries preserve the only simultaneous configuration supported by the
/// behavior contract: one accumulated modifier action plus one layer action.
/// Modified tap keys are exclusive and therefore require only one entry.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StickyKeyState {
    active: [Option<StickyEntry>; MAX_ACTIVE_STICKY_KEYS],
    canceled_modifier_releases: [Option<ModifierProducer>; MAX_STICKY_MODIFIER_PRODUCERS],
}

impl StickyKeyState {
    fn index_matching(&self, predicate: impl Fn(&StickyEntry) -> bool) -> Option<usize> {
        self.active
            .iter()
            .position(|entry| entry.as_ref().is_some_and(&predicate))
    }

    fn modifier_index(&self) -> Option<usize> {
        self.index_matching(StickyEntry::is_modifier)
    }

    fn layer_index(&self) -> Option<usize> {
        self.index_matching(StickyEntry::is_layer)
    }

    fn tap_key_index(&self) -> Option<usize> {
        self.index_matching(StickyEntry::is_tap_key)
    }

    fn insert(&mut self, entry: StickyEntry) -> Result<usize, StickyEntry> {
        let Some(index) = self.active.iter().position(Option::is_none) else {
            return Err(entry);
        };
        self.active[index] = Some(entry);
        Ok(index)
    }

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
        self.active.iter().flatten().filter_map(StickyEntry::deadline).min()
    }

    pub(crate) fn claim_buffered_press(&mut self, event: KeyboardEvent) {
        if !event.pressed {
            return;
        }
        for entry in self.active.iter_mut().flatten() {
            entry.claim_buffered_press(event.pos);
        }
    }

    pub(crate) fn finish_buffered_claim(&mut self) {
        for entry in self.active.iter_mut().flatten() {
            entry.finish_buffered_claim();
        }
    }

    fn modifier_releases_on_press(&self) -> bool {
        self.active
            .iter()
            .flatten()
            .find(|entry| entry.is_modifier())
            .is_some_and(|entry| entry.trigger_for_key(true))
    }

    pub(crate) fn modifiers(&self, pressed: bool) -> ModifierCombination {
        let mut modifiers = ModifierCombination::new();
        for entry in self.active.iter().flatten() {
            if entry.is_tap_key() || (entry.is_modifier() && (pressed || entry.phase == StickyPhase::Held)) {
                modifiers |= entry.modifiers();
            }
        }
        modifiers
    }

    fn mark_modifier_host_visible(&mut self) {
        if let Some(index) = self.modifier_index()
            && let Some(entry) = self.active[index].as_mut()
        {
            entry.modifier_effect_mut().host_visible = true;
        }
    }
}

impl Keyboard<'_> {
    pub(crate) async fn process_action_sticky_key(
        &mut self,
        action: Action,
        profile: u8,
        event: KeyboardEvent,
        event_time: Instant,
    ) {
        let shape = match action {
            Action::Modifier(_) => StickyKeyShape::PureMod,
            Action::LayerOn(_) => StickyKeyShape::Layer,
            Action::KeyWithModifier(_, _) => StickyKeyShape::TapKey,
            _ => {
                warn!("Unsupported Sticky Key action: {:?}", action);
                return;
            }
        };
        let policy = self.keymap.sticky_key_profile(profile, shape);

        // The action match is the narrow effect boundary. All variants store
        // and advance the same `StickyEntry` lifecycle.
        match action {
            Action::Modifier(modifiers) => self.process_modifier_effect(modifiers, policy, event, event_time).await,
            Action::LayerOn(layer) => self.process_layer_effect(layer, policy, event, event_time).await,
            Action::KeyWithModifier(key, modifiers) => {
                self.process_tap_key_effect(key, modifiers, policy, event, event_time)
                    .await
            }
            _ => unreachable!("unsupported Sticky Key action rejected above"),
        }
    }

    async fn process_modifier_effect(
        &mut self,
        modifiers: ModifierCombination,
        policy: StickyKeyPolicy,
        event: KeyboardEvent,
        event_time: Instant,
    ) {
        if event.pressed {
            if let Some(index) = self.sticky_key_state.tap_key_index() {
                self.release_sticky_entry(index).await;
            }
            let modifier_index = self.sticky_key_state.modifier_index();
            if modifier_index.is_some_and(|index| {
                self.sticky_key_state.active[index].is_some_and(|entry| entry.is_double_tap(event.pos, policy))
            }) {
                self.sticky_key_state
                    .remember_canceled_modifier_release(ModifierProducer {
                        source: event.pos,
                        modifiers,
                    });
                self.release_sticky_entry(modifier_index.expect("modifier checked above"))
                    .await;
                return;
            }

            if let Some(index) = modifier_index {
                let entry = self.sticky_key_state.active[index]
                    .as_mut()
                    .expect("modifier index must contain an entry");
                entry.add_modifier_producer(modifiers, event.pos);
                entry.begin_modifier_press(event.pos, policy, event_time);
            } else if self
                .sticky_key_state
                .insert(StickyEntry::new(
                    Action::Modifier(modifiers),
                    event.pos,
                    policy,
                    event_time,
                ))
                .is_err()
            {
                warn!("No capacity for Sticky modifier action");
                return;
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
            if let Some(index) = self.sticky_key_state.modifier_index() {
                let release = {
                    let entry = self.sticky_key_state.active[index]
                        .as_mut()
                        .expect("modifier index must contain an entry");
                    let exact_release = entry.on_exact_modifier_release(modifiers, event.pos);
                    exact_release
                        .map(|last| last && entry.on_physical_release(None, event_time) == PhysicalRelease::Released)
                };
                if let Some(release) = release {
                    if release {
                        self.release_sticky_entry(index).await;
                    }
                    return;
                }
            }
            if self.sticky_key_state.consume_canceled_combo_release(modifiers) {
                return;
            }
            if let Some(index) = self.sticky_key_state.modifier_index() {
                let release = {
                    let entry = self.sticky_key_state.active[index]
                        .as_mut()
                        .expect("modifier index must contain an entry");
                    entry.on_combo_modifier_release(modifiers).is_some_and(|last| last)
                        && entry.on_physical_release(None, event_time) == PhysicalRelease::Released
                };
                if release {
                    self.release_sticky_entry(index).await;
                }
            }
        }
    }

    async fn process_layer_effect(
        &mut self,
        layer: u8,
        policy: StickyKeyPolicy,
        event: KeyboardEvent,
        event_time: Instant,
    ) {
        if event.pressed {
            if let Some(index) = self.sticky_key_state.tap_key_index() {
                self.release_sticky_entry(index).await;
            }
            if layer as usize >= self.keymap.num_layer() {
                // Keep KeyMap's established diagnostic, but never arm an invalid layer.
                self.keymap.activate_layer(layer);
                return;
            }
            if let Some(index) = self.sticky_key_state.layer_index() {
                let previous = self.sticky_key_state.active[index]
                    .as_ref()
                    .expect("layer index must contain an entry");
                if previous.is_double_tap(event.pos, policy) {
                    self.release_sticky_entry(index).await;
                    return;
                }
                if previous.layer() == Some(layer) {
                    self.keymap.activate_layer(layer);
                    self.sticky_key_state.active[index]
                        .as_mut()
                        .expect("layer index must contain an entry")
                        .begin_press(event.pos, policy, event_time);
                    return;
                }
                self.release_sticky_entry(index).await;
            }
            if self
                .sticky_key_state
                .insert(StickyEntry::new(Action::LayerOn(layer), event.pos, policy, event_time))
                .is_err()
            {
                warn!("No capacity for Sticky layer action");
                return;
            }
            self.keymap.activate_layer(layer);
        } else if let Some(index) = self.sticky_key_state.layer_index() {
            let release = self.sticky_key_state.active[index]
                .as_mut()
                .expect("layer index must contain an entry")
                .on_physical_release(Some(event.pos), event_time)
                == PhysicalRelease::Released;
            if release {
                self.release_sticky_entry(index).await;
            }
        }
    }

    async fn process_tap_key_effect(
        &mut self,
        key: HidKeyCode,
        modifiers: ModifierCombination,
        policy: StickyKeyPolicy,
        event: KeyboardEvent,
        event_time: Instant,
    ) {
        let action = Action::KeyWithModifier(key, modifiers);

        if event.pressed {
            for index in 0..MAX_ACTIVE_STICKY_KEYS {
                if self.sticky_key_state.active[index].is_some_and(|entry| !entry.is_tap_key()) {
                    self.release_sticky_entry(index).await;
                }
            }

            let tap_key_index = self.sticky_key_state.tap_key_index();
            let same_tap_key = tap_key_index.is_some_and(|index| {
                self.sticky_key_state.active[index]
                    .is_some_and(|entry| entry.source == event.pos && entry.action == action)
            });
            if same_tap_key
                && tap_key_index.is_some_and(|index| {
                    self.sticky_key_state.active[index].is_some_and(|entry| entry.is_double_tap(event.pos, policy))
                })
            {
                self.release_sticky_entry(tap_key_index.expect("tap key checked above"))
                    .await;
                return;
            }
            if !same_tap_key && let Some(index) = tap_key_index {
                self.release_sticky_entry(index).await;
            }

            let mut deactivate = false;
            if let Some(index) = self.sticky_key_state.tap_key_index() {
                let entry = self.sticky_key_state.active[index]
                    .as_mut()
                    .expect("tap key index must contain an entry");
                entry.repeat_count = entry.repeat_count.saturating_add(1);
                if policy.max_repeat > 0 && entry.repeat_count > policy.max_repeat {
                    deactivate = true;
                } else {
                    entry.begin_press(event.pos, policy, event_time);
                }
            } else if self
                .sticky_key_state
                .insert(StickyEntry::new(action, event.pos, policy, event_time))
                .is_err()
            {
                warn!("No capacity for Sticky tap-key action");
                return;
            }

            if deactivate {
                self.release_sticky_entry(
                    self.sticky_key_state
                        .tap_key_index()
                        .expect("tap key must exist before repeat deactivation"),
                )
                .await;
            } else {
                self.process_action_key(key, event).await;
            }
        } else if let Some(index) = self.sticky_key_state.tap_key_index() {
            let release_physical_key = self.sticky_key_state.active[index].is_some_and(|entry| {
                entry.source == event.pos
                    && matches!(entry.phase, StickyPhase::Pressed | StickyPhase::PressDeadlineInactive)
            });
            if release_physical_key {
                self.sticky_key_state.active[index]
                    .as_mut()
                    .expect("tap key index must contain an entry")
                    .on_physical_release(Some(event.pos), event_time);
                self.process_action_key(key, event).await;
            }
        }
    }

    /// Release a tap-key entry around a foreign action at the ordering boundary
    /// required by its policy. Shape checks remain internal to Sticky Key.
    pub(crate) async fn prepare_sticky_key_for_action(
        &mut self,
        action: Action,
        event: KeyboardEvent,
    ) -> StickyKeyPostAction {
        let Some(index) = self.sticky_key_state.tap_key_index() else {
            return StickyKeyPostAction::None;
        };
        let preserves_tap_key = matches!(action, Action::Modifier(_))
            || matches!(action, Action::Key(KeyCode::Hid(key)) if key.is_modifier());
        let releases = self.sticky_key_state.active[index]
            .is_some_and(|entry| !preserves_tap_key && entry.trigger_for_key(event.pressed));
        if releases && event.pressed {
            self.release_sticky_entry(index).await;
            StickyKeyPostAction::None
        } else if releases {
            StickyKeyPostAction::Release
        } else {
            StickyKeyPostAction::None
        }
    }

    pub(crate) async fn finish_sticky_key_after_action(&mut self, post_action: StickyKeyPostAction) {
        if post_action == StickyKeyPostAction::Release
            && let Some(index) = self.sticky_key_state.tap_key_index()
        {
            self.release_sticky_entry(index).await;
        }
    }

    /// Apply a foreign key event uniformly to all active entries.
    fn update_sticky_key(&mut self, event: KeyboardEvent) -> StickyKeyUpdate {
        let mut update = StickyKeyUpdate::default();

        for index in 0..MAX_ACTIVE_STICKY_KEYS {
            let Some(entry) = self.sticky_key_state.active[index].as_mut() else {
                continue;
            };
            if entry.is_tap_key() {
                continue;
            }
            match entry.phase {
                StickyPhase::Pressed | StickyPhase::PressDeadlineInactive | StickyPhase::HoldQualified => {
                    entry.mark_foreign_key()
                }
                StickyPhase::Latched if entry.trigger_for_key(event.pressed) => {
                    let entry = self.sticky_key_state.active[index]
                        .take()
                        .expect("active entry checked above");
                    match entry.action {
                        Action::Modifier(_) => {
                            update.modifier_was_host_visible = entry.modifier_effect().host_visible;
                            update.modifier_consumed = true;
                        }
                        Action::LayerOn(layer) => {
                            self.keymap.deactivate_layer_if_active(layer);
                        }
                        _ => unreachable!("tap-key and unsupported entries are not foreign-key candidates"),
                    }
                }
                StickyPhase::Latched | StickyPhase::Held => {}
            }
        }
        update
    }

    /// Finish Sticky Key handling after a concrete key report has been sent.
    /// The result tells the caller whether a balancing keyboard report is
    /// needed; modifier/layer/tap-key details remain inside this module.
    pub(crate) fn finish_sticky_key_for_key(&mut self, event: KeyboardEvent, is_basic_keyboard_key: bool) -> bool {
        if is_basic_keyboard_key {
            self.sticky_key_state.mark_modifier_host_visible();
        }
        let modifier_releases_on_press = self.sticky_key_state.modifier_releases_on_press();
        let update = self.update_sticky_key(event);

        (is_basic_keyboard_key && event.pressed && modifier_releases_on_press && update.modifier_consumed)
            || (!is_basic_keyboard_key && update.modifier_consumed && update.modifier_was_host_visible)
    }

    pub(crate) async fn release_sticky_key_on_layer_event(&mut self, event: StickyKeyReleaseMode) {
        for index in 0..MAX_ACTIVE_STICKY_KEYS {
            if self.sticky_key_state.active[index].is_some_and(|entry| entry.policy.release_mode.intersects(event)) {
                self.release_sticky_entry(index).await;
            }
        }
    }

    pub(crate) async fn release_sticky_key_if_active_on_timeout(&mut self) {
        let now = Instant::now();
        for index in 0..MAX_ACTIVE_STICKY_KEYS {
            let release = self.sticky_key_state.active[index]
                .as_mut()
                .is_some_and(|entry| entry.deadline_disposition(now) == DeadlineDisposition::Release);
            if release {
                self.release_sticky_entry(index).await;
            }
        }
    }

    /// Remove one entry and release the concrete device effect it owns.
    async fn release_sticky_entry(&mut self, index: usize) {
        let Some(entry) = self.sticky_key_state.active[index].take() else {
            return;
        };
        match entry.action {
            Action::Modifier(_) => {
                let StickyEffectState::Modifier(modifier) = entry.effect_state else {
                    unreachable!("modifier action must own modifier effect state");
                };
                for producer in modifier.producers.into_iter().flatten() {
                    self.sticky_key_state.remember_canceled_modifier_release(producer);
                }
                if modifier.host_visible {
                    self.send_keyboard_report_with_resolved_modifiers(false).await;
                }
            }
            Action::LayerOn(layer) => {
                self.keymap.deactivate_layer_if_active(layer);
            }
            Action::KeyWithModifier(key, _) => {
                if matches!(entry.phase, StickyPhase::Pressed | StickyPhase::PressDeadlineInactive) {
                    self.process_action_key(
                        key,
                        KeyboardEvent {
                            pressed: false,
                            pos: entry.source,
                        },
                    )
                    .await;
                } else {
                    self.send_keyboard_report_with_resolved_modifiers(false).await;
                }
            }
            _ => unreachable!("unsupported action cannot enter Sticky Key state"),
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
            release_after_hold: StickyKeyHoldDuration::DISABLED,
            max_repeat: 0,
            release_mode,
        }
    }

    fn modifier_entry(
        modifiers: ModifierCombination,
        source: KeyboardEventPos,
        policy: StickyKeyPolicy,
        pressed_at: Instant,
    ) -> StickyEntry {
        StickyEntry::new(Action::Modifier(modifiers), source, policy, pressed_at)
    }

    #[test]
    fn modifier_effect_counts_overlapping_physical_producers() {
        let pressed_at = Instant::now();
        let mut latch = modifier_entry(
            ModifierCombination::LCTRL,
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
            pressed_at,
        );
        latch.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
        latch.begin_modifier_press(pos(1), policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE), pressed_at);

        assert_eq!(
            latch.on_exact_modifier_release(ModifierCombination::LCTRL, pos(0)),
            Some(false)
        );
        assert_eq!(latch.phase, StickyPhase::PressDeadlineInactive);
        assert_eq!(
            latch.on_exact_modifier_release(ModifierCombination::LSHIFT, pos(1)),
            Some(true)
        );
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Latched
        );
        assert_eq!(latch.phase, StickyPhase::Latched);
        assert_eq!(
            latch.modifiers(),
            ModifierCombination::LCTRL | ModifierCombination::LSHIFT
        );
    }

    #[test]
    fn held_latch_releases_after_last_physical_producer() {
        let pressed_at = Instant::now();
        let mut latch = modifier_entry(
            ModifierCombination::LCTRL,
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
            pressed_at,
        );
        latch.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
        latch.begin_modifier_press(pos(1), policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE), pressed_at);
        latch.mark_foreign_key();

        assert_eq!(
            latch.on_exact_modifier_release(ModifierCombination::LCTRL, pos(0)),
            Some(false)
        );
        assert_eq!(
            latch.on_exact_modifier_release(ModifierCombination::LSHIFT, pos(1)),
            Some(true)
        );
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Released
        );
    }

    #[test]
    fn disabled_hold_threshold_has_no_press_deadline() {
        let pressed_at = Instant::now();
        let mut latch = modifier_entry(
            ModifierCombination::LCTRL,
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
            pressed_at,
        );

        assert_eq!(latch.phase, StickyPhase::PressDeadlineInactive);
        assert_eq!(latch.deadline(), None);
        assert_eq!(latch.timing_marker, Some(pressed_at));
        assert_eq!(
            latch.on_physical_release(None, pressed_at + Duration::from_secs(2)),
            PhysicalRelease::Latched
        );
        assert_eq!(latch.phase, StickyPhase::Latched);
        assert!(latch.timing_marker.is_some());
    }

    #[test]
    fn configured_hold_threshold_releases_after_deferred_timeout() {
        let mut hold_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        hold_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_millis(300));
        let mut latch = modifier_entry(ModifierCombination::LCTRL, pos(0), hold_policy, Instant::now());
        let deadline = latch.timing_marker.unwrap();

        assert_eq!(latch.deadline_disposition(deadline), DeadlineDisposition::Deferred);
        assert_eq!(latch.phase, StickyPhase::HoldQualified);
        assert_eq!(latch.on_physical_release(None, deadline), PhysicalRelease::Released);
        assert_eq!(latch.timing_marker, None);
    }

    #[test]
    fn configured_hold_threshold_is_inclusive() {
        let mut hold_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        hold_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_millis(300));
        let mut short_press = modifier_entry(ModifierCombination::LCTRL, pos(0), hold_policy, Instant::now());
        let threshold = short_press.timing_marker.unwrap();
        let just_before_threshold = threshold - Duration::from_millis(1);

        assert_eq!(
            short_press.on_physical_release(None, just_before_threshold),
            PhysicalRelease::Latched
        );
        assert_eq!(short_press.phase, StickyPhase::Latched);
        assert_eq!(
            short_press.timing_marker,
            Some(just_before_threshold + hold_policy.timeout)
        );

        let mut threshold_press = modifier_entry(ModifierCombination::LCTRL, pos(0), hold_policy, Instant::now());
        let threshold = threshold_press.timing_marker.unwrap();

        assert_eq!(
            threshold_press.on_physical_release(None, threshold),
            PhysicalRelease::Released
        );
        assert_eq!(threshold_press.timing_marker, None);
    }

    #[test]
    fn hold_threshold_may_exceed_the_latched_timeout() {
        let mut hold_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        hold_policy.timeout = Duration::from_millis(300);
        hold_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_secs(1));
        let pressed_at = Instant::now();
        let released_at = pressed_at + Duration::from_millis(600);
        let mut latch = modifier_entry(ModifierCombination::LCTRL, pos(0), hold_policy, pressed_at);

        assert_eq!(latch.on_physical_release(None, released_at), PhysicalRelease::Latched);
        assert_eq!(latch.phase, StickyPhase::Latched);
        assert_eq!(latch.timing_marker, Some(released_at + hold_policy.timeout));
    }

    #[test]
    fn hold_threshold_uses_original_buffered_press_time() {
        let mut hold_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        hold_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_millis(300));
        let pressed_at = Instant::now();
        let dispatched_at = pressed_at + Duration::from_millis(250);
        let mut latch = modifier_entry(ModifierCombination::LCTRL, pos(0), hold_policy, pressed_at);

        assert_eq!(
            latch.on_physical_release(None, dispatched_at + Duration::from_millis(50)),
            PhysicalRelease::Released
        );
    }

    #[test]
    fn overlapping_modifier_preserves_chord_hold_start() {
        let mut hold_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        hold_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_millis(300));
        let first_press = Instant::now();
        let second_press = first_press + Duration::from_millis(250);
        let release_time = first_press + Duration::from_millis(400);
        let mut latch = modifier_entry(ModifierCombination::LCTRL, pos(0), hold_policy, first_press);
        latch.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
        latch.begin_modifier_press(pos(1), hold_policy, second_press);

        assert_eq!(
            latch.on_exact_modifier_release(ModifierCombination::LSHIFT, pos(1)),
            Some(false)
        );
        assert_eq!(
            latch.on_exact_modifier_release(ModifierCombination::LCTRL, pos(0)),
            Some(true)
        );
        assert_eq!(latch.on_physical_release(None, release_time), PhysicalRelease::Released);

        let mut reverse_release = modifier_entry(ModifierCombination::LCTRL, pos(0), hold_policy, first_press);
        reverse_release.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
        reverse_release.begin_modifier_press(pos(1), hold_policy, second_press);
        assert_eq!(
            reverse_release.on_exact_modifier_release(ModifierCombination::LCTRL, pos(0)),
            Some(false)
        );
        assert_eq!(
            reverse_release.on_exact_modifier_release(ModifierCombination::LSHIFT, pos(1)),
            Some(true)
        );
        assert_eq!(
            reverse_release.on_physical_release(None, release_time),
            PhysicalRelease::Released
        );
    }

    #[test]
    fn latest_modifier_policy_uses_chord_hold_start() {
        let mut first_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        first_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_millis(300));
        let mut latest_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        latest_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_millis(500));
        let first_press = Instant::now();
        let mut latch = modifier_entry(ModifierCombination::LCTRL, pos(0), first_policy, first_press);
        latch.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
        latch.begin_modifier_press(pos(1), latest_policy, first_press + Duration::from_millis(250));

        assert_eq!(
            latch.on_physical_release(None, first_press + Duration::from_millis(400)),
            PhysicalRelease::Latched
        );

        let mut timeout_first = modifier_entry(ModifierCombination::LCTRL, pos(0), first_policy, first_press);
        assert_eq!(
            timeout_first.deadline_disposition(first_press + Duration::from_millis(300)),
            DeadlineDisposition::Deferred
        );
        timeout_first.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
        timeout_first.begin_modifier_press(pos(1), latest_policy, first_press + Duration::from_millis(400));
        assert_eq!(
            timeout_first.on_physical_release(None, first_press + Duration::from_millis(450)),
            PhysicalRelease::Latched
        );
    }

    #[test]
    fn mixed_modifier_profiles_are_independent_of_deadline_poll_order() {
        let mut enabled_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        enabled_policy.release_after_hold = StickyKeyHoldDuration::from_duration(Duration::from_millis(500));
        let mut disabled_policy = policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE);
        disabled_policy.timeout = Duration::from_millis(300);
        let first_press = Instant::now();

        for poll_first in [false, true] {
            let mut disabled_then_enabled =
                modifier_entry(ModifierCombination::LCTRL, pos(0), disabled_policy, first_press);
            if poll_first {
                assert_eq!(
                    disabled_then_enabled.deadline_disposition(first_press + Duration::from_millis(300)),
                    DeadlineDisposition::Pending
                );
            }
            disabled_then_enabled.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
            disabled_then_enabled.begin_modifier_press(
                pos(1),
                enabled_policy,
                first_press + Duration::from_millis(400),
            );
            assert_eq!(
                disabled_then_enabled.on_physical_release(None, first_press + Duration::from_millis(450)),
                PhysicalRelease::Latched
            );

            let mut enabled_then_disabled =
                modifier_entry(ModifierCombination::LCTRL, pos(0), enabled_policy, first_press);
            if poll_first {
                assert_eq!(
                    enabled_then_disabled.deadline_disposition(first_press + Duration::from_millis(500)),
                    DeadlineDisposition::Deferred
                );
            }
            enabled_then_disabled.add_modifier_producer(ModifierCombination::LSHIFT, pos(1));
            enabled_then_disabled.begin_modifier_press(
                pos(1),
                disabled_policy,
                first_press + Duration::from_millis(600),
            );
            assert_eq!(
                enabled_then_disabled.on_physical_release(None, first_press + Duration::from_millis(650)),
                PhysicalRelease::Latched
            );
        }
    }

    #[test]
    fn buffered_foreign_press_claims_latch_until_action_resolves() {
        let mut latch = modifier_entry(
            ModifierCombination::LCTRL,
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_PRESS),
            Instant::now(),
        );
        assert_eq!(
            latch.on_physical_release(None, Instant::now()),
            PhysicalRelease::Latched
        );

        latch.claim_buffered_press(pos(1));
        assert!(latch.buffered_claim);
        assert_eq!(latch.timing_marker, None);

        latch.finish_buffered_claim();
        assert!(!latch.buffered_claim);
        assert!(latch.timing_marker.is_some());
    }

    #[test]
    fn active_state_stores_actions_in_one_uniform_collection() {
        let mut state = StickyKeyState::default();
        let modifier = modifier_entry(
            ModifierCombination::LCTRL,
            pos(0),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
            Instant::now(),
        );
        let layer = StickyEntry::new(
            Action::LayerOn(2),
            pos(1),
            policy(StickyKeyReleaseMode::LAYER_EXIT),
            Instant::now(),
        );

        state.insert(modifier).unwrap();
        state.insert(layer).unwrap();

        assert!(
            state
                .active
                .iter()
                .flatten()
                .any(|entry| { entry.action == Action::Modifier(ModifierCombination::LCTRL) })
        );
        assert!(
            state
                .active
                .iter()
                .flatten()
                .any(|entry| entry.action == Action::LayerOn(2))
        );

        let overflow = StickyEntry::new(
            Action::KeyWithModifier(HidKeyCode::A, ModifierCombination::LSHIFT),
            pos(2),
            policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
            Instant::now(),
        );
        assert_eq!(state.insert(overflow).unwrap_err().action, overflow.action);
    }

    #[test]
    fn canceled_release_does_not_consume_a_new_modifier_latch() {
        let mut state = StickyKeyState::default();
        state.remember_canceled_modifier_release(ModifierProducer {
            source: pos(0),
            modifiers: ModifierCombination::LSHIFT,
        });
        state
            .insert(modifier_entry(
                ModifierCombination::LCTRL,
                pos(1),
                policy(StickyKeyReleaseMode::OTHER_KEY_RELEASE),
                Instant::now(),
            ))
            .unwrap();

        assert!(state.consume_exact_canceled_modifier_release(ModifierCombination::LSHIFT, pos(0)));
        assert!(state.modifier_index().is_some());
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
