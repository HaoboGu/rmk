//! Auto mouse layer
//!
//! Subscribes to [`PointingEvent`]s coming from pointing devices (e.g. PMW3610)
//! and activates a configured layer whenever cursor motion (X/Y axis) is
//! detected.
//!
//! Multiple entries can be configured to give each pointing device its own
//! target_layer/threshold/timeout. For each event the entry whose `device_id` matches
//! the event's `device_id` is selected first; otherwise the entry with
//! `device_id == None` (if any) is used as a fallback. Events that match
//! neither are ignored.

use embassy_time::Instant;
use heapless::Vec;
use rmk_macro::processor;

use crate::AUTO_MOUSE_LAYER_MAX_NUM;
use crate::config::AutoMouseLayerConfig;
use crate::core_traits::Runnable;
use crate::event::{Axis, AxisValType, LayerChangeEvent, PointingEvent};
use crate::keymap::KeyMap;
use crate::processor::DeadlineProcessor;

/// [`Runnable`] for the auto mouse layer task.
///
/// Construct with [`AutoMouseLayerRunner::new`] and pass to `run_all!`. If the
/// keymap has no auto mouse layer configured (or every entry's layer is out of
/// range), [`Runnable::run`] parks forever on [`core::future::pending`] so it
/// can sit alongside the other tasks without doing anything.
#[processor(subscribe = [PointingEvent, LayerChangeEvent])]
#[::rmk::macros::runnable_generated]
pub struct AutoMouseLayerRunner<'a, 'k> {
    keymap: &'a KeyMap<'k>,
    entries: Vec<EntryState, AUTO_MOUSE_LAYER_MAX_NUM>,
}

impl<'a, 'k> AutoMouseLayerRunner<'a, 'k> {
    /// Build the runner from the keymap's `[behavior.auto_mouse_layer]` config.
    pub fn new(keymap: &'a KeyMap<'k>) -> Self {
        let num_layer = keymap.num_layer();
        let configs = keymap.auto_mouse_layer_configs();
        let mut entries: Vec<EntryState, AUTO_MOUSE_LAYER_MAX_NUM> = Vec::new();
        for config in configs.iter().copied() {
            if (config.target_layer as usize) >= num_layer {
                warn!(
                    "auto_mouse_layer: configured target_layer {} is out of range (keymap has {} layers); \
                     entry for device_id {:?} will be ignored",
                    config.target_layer, num_layer, config.device_id
                );
                continue;
            }
            // threshold == 0 would short-circuit motion detection — guard against
            // a misconfigured Rust-API caller bypassing AutoMouseLayerConfig::new.
            let mut config = config;
            config.threshold = config.threshold.max(1);
            // Capacity already matches AUTO_MOUSE_LAYER_MAX_NUM upstream.
            let _ = entries.push(EntryState {
                config,
                self_activated: false,
                deadline: None,
                overlap_warned: false,
            });
        }
        Self { keymap, entries }
    }

    async fn on_pointing_event(&mut self, event: PointingEvent) {
        let Some(idx) = match_entry(&self.entries, event.device_id) else {
            return;
        };
        if !is_cursor_motion(&event, self.entries[idx].config.threshold) {
            return;
        }
        let target_layer = self.entries[idx].config.target_layer;
        let activated_by_us = self.keymap.activate_layer_if_inactive(target_layer);
        if pointing_step(&mut self.entries, idx, Instant::now(), activated_by_us) == PointingOutcome::OverlapFirstSeen {
            warn!(
                "auto_mouse_layer: layer {} is already active when motion was detected; \
                 the layer is likely driven by another key (MO/TG). The auto mouse layer \
                 will not be deactivated on timeout while overlap holds.",
                target_layer
            );
        }
    }

    async fn on_layer_change_event(&mut self, LayerChangeEvent(top): LayerChangeEvent) {
        // Layer turned off externally (MO/TG key etc.) — release our hold.
        let keymap = self.keymap;
        for entry in self.entries.iter_mut() {
            if entry.self_activated && !keymap.is_layer_active(entry.config.target_layer) {
                entry.self_activated = false;
                entry.deadline = None;
                trace!(
                    "auto_mouse_layer: cleared tracking for layer {} (top now {})",
                    entry.config.target_layer, top
                );
            }
        }
    }
}

/// Per-entry runtime state.
#[derive(Clone, Copy)]
struct EntryState {
    config: AutoMouseLayerConfig,
    /// `true` while this entry is holding the layer active. Multiple entries
    /// may hold the same layer simultaneously when they share `target_layer`.
    self_activated: bool,
    /// Set when the entry is self-activated; the layer is deactivated when this
    /// time is reached unless further motion pushes the deadline forward.
    deadline: Option<Instant>,
    /// Whether we have already warned about the entry's layer overlapping a
    /// manually-activated layer.
    overlap_warned: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum PointingOutcome {
    Holding,
    OverlapFirstSeen,
    Idle,
}

impl DeadlineProcessor for AutoMouseLayerRunner<'_, '_> {
    fn deadline(&self) -> Option<Instant> {
        earliest_deadline(&self.entries)
    }

    async fn on_deadline(&mut self) {
        for layer in timeout_step(&mut self.entries, Instant::now()) {
            self.keymap.deactivate_layer_if_active(layer);
        }
    }
}

impl Runnable for AutoMouseLayerRunner<'_, '_> {
    async fn run(&mut self) -> ! {
        if self.entries.is_empty() {
            core::future::pending().await
        }
        self.deadline_loop().await
    }
}

fn earliest_deadline(entries: &[EntryState]) -> Option<Instant> {
    entries.iter().filter_map(|e| e.deadline).min()
}

/// Find the entry that should handle an event from `device_id`.
///
/// Exact `device_id` match wins; otherwise the first entry with
/// `device_id == None` is used as a fallback. Returns `None` if the event
/// matches neither.
fn match_entry(entries: &[EntryState], device_id: u8) -> Option<usize> {
    if let Some(i) = entries.iter().position(|e| e.config.device_id == Some(device_id)) {
        return Some(i);
    }
    entries.iter().position(|e| e.config.device_id.is_none())
}

fn layer_still_held(entries: &[EntryState], layer: u8) -> bool {
    entries
        .iter()
        .any(|e| e.self_activated && e.config.target_layer == layer)
}

/// Whether some entry other than `idx` self-holds `layer`.
fn layer_shared_with_other(entries: &[EntryState], idx: usize, layer: u8) -> bool {
    entries
        .iter()
        .enumerate()
        .any(|(i, e)| i != idx && e.self_activated && e.config.target_layer == layer)
}

fn timeout_step(entries: &mut [EntryState], now: Instant) -> Vec<u8, AUTO_MOUSE_LAYER_MAX_NUM> {
    let mut released: Vec<u8, AUTO_MOUSE_LAYER_MAX_NUM> = Vec::new();
    for i in 0..entries.len() {
        let expired = entries[i].self_activated && entries[i].deadline.is_some_and(|d| d <= now);
        if !expired {
            continue;
        }
        entries[i].self_activated = false;
        entries[i].deadline = None;
        let layer = entries[i].config.target_layer;
        if !layer_still_held(entries, layer) {
            let _ = released.push(layer);
        }
    }
    released
}

fn pointing_step(entries: &mut [EntryState], idx: usize, now: Instant, activated_by_us: bool) -> PointingOutcome {
    let target_layer = entries[idx].config.target_layer;
    let shared_with_other = layer_shared_with_other(entries, idx, target_layer);
    let entry = &mut entries[idx];
    if activated_by_us || shared_with_other {
        entry.self_activated = true;
        entry.overlap_warned = false;
        entry.deadline = Some(now + entry.config.timeout);
        PointingOutcome::Holding
    } else if entry.self_activated {
        entry.deadline = Some(now + entry.config.timeout);
        PointingOutcome::Idle
    } else if !entry.overlap_warned {
        entry.overlap_warned = true;
        PointingOutcome::OverlapFirstSeen
    } else {
        PointingOutcome::Idle
    }
}

/// Only relative X/Y axis deltas count as cursor motion. Scroll-only events
/// (Z/H/V) do not activate the layer.
///
/// Absolute-position axes ([`AxisValType::Abs`], e.g. analogue joysticks) are
/// also ignored here: their `value` reports the current position rather than a
/// delta, so a stick held off-centre would keep the layer pinned on forever.
/// Absolute pointing devices need to be converted to relative deltas upstream.
fn is_cursor_motion(event: &PointingEvent, threshold: u16) -> bool {
    event.axes.iter().any(|axis| {
        matches!(axis.typ, AxisValType::Rel)
            && matches!(axis.axis, Axis::X | Axis::Y)
            && axis.value.unsigned_abs() >= threshold
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{AxisEvent, AxisValType};

    fn axis(axis: Axis, value: i16) -> AxisEvent {
        AxisEvent {
            typ: AxisValType::Rel,
            axis,
            value,
        }
    }

    fn abs_axis(axis: Axis, value: i16) -> AxisEvent {
        AxisEvent {
            typ: AxisValType::Abs,
            axis,
            value,
        }
    }

    fn event(axes: [AxisEvent; 3]) -> PointingEvent {
        event_for(0, axes)
    }

    fn event_for(device_id: u8, axes: [AxisEvent; 3]) -> PointingEvent {
        PointingEvent { device_id, axes }
    }

    fn entry(device_id: Option<u8>) -> EntryState {
        EntryState {
            config: AutoMouseLayerConfig {
                device_id,
                target_layer: 0,
                timeout: embassy_time::Duration::from_millis(100),
                threshold: 1,
            },
            self_activated: false,
            deadline: None,
            overlap_warned: false,
        }
    }

    #[test]
    fn is_cursor_motion_detects_x_or_y() {
        let zero = axis(Axis::Z, 0);
        assert!(is_cursor_motion(&event([axis(Axis::X, 5), zero, zero]), 1u16));
        assert!(is_cursor_motion(&event([zero, axis(Axis::Y, -3), zero]), 1u16));
    }

    #[test]
    fn is_cursor_motion_ignores_scroll_axes() {
        let zero = axis(Axis::X, 0);
        assert!(!is_cursor_motion(&event([zero, zero, axis(Axis::Z, 100)]), 1u16));
        assert!(!is_cursor_motion(&event([zero, zero, axis(Axis::V, 100)]), 1u16));
        assert!(!is_cursor_motion(&event([zero, zero, axis(Axis::H, 100)]), 1u16));
    }

    #[test]
    fn is_cursor_motion_applies_threshold() {
        let zero = axis(Axis::Y, 0);
        assert!(!is_cursor_motion(&event([axis(Axis::X, 2), zero, zero]), 3u16));
        assert!(is_cursor_motion(&event([axis(Axis::X, 3), zero, zero]), 3u16));
    }

    #[test]
    fn is_cursor_motion_detects_when_both_x_and_y_present() {
        // X and Y in the same event are checked independently; either one
        // crossing the threshold triggers motion.
        let zero_z = axis(Axis::Z, 0);
        assert!(is_cursor_motion(
            &event([axis(Axis::X, 4), axis(Axis::Y, 7), zero_z]),
            3u16
        ));
        // Sub-threshold X paired with above-threshold Y still triggers.
        assert!(is_cursor_motion(
            &event([axis(Axis::X, 1), axis(Axis::Y, 5), zero_z]),
            3u16
        ));
        // Both sub-threshold: deltas are NOT summed, so no motion.
        assert!(!is_cursor_motion(
            &event([axis(Axis::X, 2), axis(Axis::Y, 2), zero_z]),
            3u16
        ));
    }

    #[test]
    fn is_cursor_motion_ignores_absolute_component_when_mixed_with_relative() {
        // Above-threshold Abs X must not trigger even when paired with a
        // sub-threshold Rel Y: only Rel deltas count.
        let zero_z = axis(Axis::Z, 0);
        assert!(!is_cursor_motion(
            &event([abs_axis(Axis::X, i16::MAX), axis(Axis::Y, 1), zero_z]),
            3u16
        ));
        // ... but an above-threshold Rel Y in the same event does trigger.
        assert!(is_cursor_motion(
            &event([abs_axis(Axis::X, i16::MAX), axis(Axis::Y, 5), zero_z]),
            3u16
        ));
    }

    #[test]
    fn is_cursor_motion_ignores_absolute_axes() {
        // A joystick reporting an off-centre position via absolute X/Y must not
        // be treated as motion — otherwise the layer would stick on forever
        // while the stick is held.
        let zero = axis(Axis::Y, 0);
        assert!(!is_cursor_motion(
            &event([abs_axis(Axis::X, i16::MAX), zero, zero]),
            1u16
        ));
        assert!(!is_cursor_motion(&event([zero, abs_axis(Axis::Y, -32000), zero]), 1u16));
    }

    #[test]
    fn match_entry_prefers_exact_device_id() {
        // Order intentionally puts the fallback first to confirm the exact
        // match wins regardless of position.
        let entries = [entry(None), entry(Some(1)), entry(Some(2))];
        assert_eq!(match_entry(&entries, 1), Some(1));
        assert_eq!(match_entry(&entries, 2), Some(2));
    }

    #[test]
    fn match_entry_falls_back_to_no_device_id() {
        let entries = [entry(Some(1)), entry(None), entry(Some(2))];
        // device_id 7 is configured nowhere → fallback entry (index 1).
        assert_eq!(match_entry(&entries, 7), Some(1));
    }

    #[test]
    fn match_entry_returns_none_when_no_match_and_no_fallback() {
        let entries = [entry(Some(1)), entry(Some(2))];
        assert_eq!(match_entry(&entries, 7), None);
    }

    fn active_entry(device_id: Option<u8>, target_layer: u8) -> EntryState {
        let mut e = entry(device_id);
        e.config.target_layer = target_layer;
        e.self_activated = true;
        e
    }

    #[test]
    fn layer_still_held_true_when_any_entry_self_holds_the_layer() {
        let entries = [active_entry(Some(1), 3), entry(Some(2))];
        assert!(layer_still_held(&entries, 3));
    }

    #[test]
    fn layer_still_held_false_when_no_entry_active_on_that_layer() {
        let entries = [entry(Some(1)), active_entry(Some(2), 5)];
        assert!(!layer_still_held(&entries, 3));
    }

    #[test]
    fn layer_still_held_ignores_entries_not_self_activated() {
        let mut e = entry(Some(1));
        e.config.target_layer = 2;
        assert!(!layer_still_held(&[e], 2));
    }

    #[test]
    fn layer_shared_with_other_true_when_another_self_activated_entry_holds_same_layer() {
        let entries = [active_entry(Some(1), 2), active_entry(Some(2), 2)];
        assert!(layer_shared_with_other(&entries, 0, 2));
    }

    #[test]
    fn layer_shared_with_other_excludes_self_index() {
        let entries = [active_entry(Some(1), 2), entry(Some(2))];
        assert!(!layer_shared_with_other(&entries, 0, 2));
    }

    #[test]
    fn layer_shared_with_other_ignores_other_layers() {
        let entries = [active_entry(Some(1), 2), active_entry(Some(2), 5)];
        assert!(!layer_shared_with_other(&entries, 0, 2));
    }

    #[test]
    fn event_for_carries_device_id() {
        // Smoke test confirming the test helper does what later integration
        // tests would rely on.
        let zero = axis(Axis::Z, 0);
        let e = event_for(3, [axis(Axis::X, 5), axis(Axis::Y, 0), zero]);
        assert_eq!(e.device_id, 3);
        assert!(is_cursor_motion(&e, 1));
    }

    fn at(millis: u64) -> Instant {
        Instant::from_millis(millis)
    }

    fn entry_with_layer(device_id: Option<u8>, target_layer: u8) -> EntryState {
        let mut e = entry(device_id);
        e.config.target_layer = target_layer;
        e
    }

    #[test]
    fn timeout_step_releases_layer_when_last_holder_expires() {
        let mut entries = [entry_with_layer(Some(1), 3)];
        entries[0].self_activated = true;
        entries[0].deadline = Some(at(100));

        let released = timeout_step(&mut entries, at(150));

        assert_eq!(released.as_slice(), &[3]);
        assert!(!entries[0].self_activated);
        assert!(entries[0].deadline.is_none());
    }

    #[test]
    fn timeout_step_keeps_shared_layer_alive_while_a_sibling_still_holds_it() {
        let mut entries = [entry_with_layer(Some(1), 2), entry_with_layer(Some(2), 2)];
        entries[0].self_activated = true;
        entries[0].deadline = Some(at(50));
        entries[1].self_activated = true;
        entries[1].deadline = Some(at(500));

        let released = timeout_step(&mut entries, at(100));

        assert!(released.is_empty());
        assert!(!entries[0].self_activated);
        assert!(entries[0].deadline.is_none());
        assert!(entries[1].self_activated);
        assert_eq!(entries[1].deadline, Some(at(500)));
    }

    #[test]
    fn timeout_step_releases_shared_layer_when_all_holders_expire_simultaneously() {
        let mut entries = [entry_with_layer(Some(1), 4), entry_with_layer(Some(2), 4)];
        entries[0].self_activated = true;
        entries[0].deadline = Some(at(50));
        entries[1].self_activated = true;
        entries[1].deadline = Some(at(80));

        let released = timeout_step(&mut entries, at(100));

        assert_eq!(released.as_slice(), &[4]);
        assert!(!entries[0].self_activated);
        assert!(!entries[1].self_activated);
    }

    #[test]
    fn timeout_step_ignores_entries_that_are_not_self_activated() {
        let mut entries = [entry_with_layer(Some(1), 1)];
        entries[0].self_activated = false;
        entries[0].deadline = Some(at(10));

        let released = timeout_step(&mut entries, at(500));

        assert!(released.is_empty());
    }

    #[test]
    fn pointing_step_holds_layer_when_activation_succeeds() {
        let mut entries = [entry_with_layer(Some(1), 2)];

        let outcome = pointing_step(&mut entries, 0, at(1000), true);

        assert_eq!(outcome, PointingOutcome::Holding);
        assert!(entries[0].self_activated);
        assert_eq!(entries[0].deadline, Some(at(1000) + entries[0].config.timeout));
        assert!(!entries[0].overlap_warned);
    }

    #[test]
    fn pointing_step_piggybacks_on_a_sibling_that_already_holds_the_shared_layer() {
        let mut entries = [entry_with_layer(Some(1), 2), entry_with_layer(Some(2), 2)];
        entries[0].self_activated = true;
        entries[0].deadline = Some(at(500));

        let outcome = pointing_step(&mut entries, 1, at(1000), false);

        assert_eq!(outcome, PointingOutcome::Holding);
        assert!(entries[1].self_activated);
        assert_eq!(entries[1].deadline, Some(at(1000) + entries[1].config.timeout));
        assert!(entries[0].self_activated);
        assert_eq!(entries[0].deadline, Some(at(500)));
    }

    #[test]
    fn pointing_step_warns_once_when_layer_is_externally_active() {
        let mut entries = [entry_with_layer(Some(1), 2)];

        let first = pointing_step(&mut entries, 0, at(1000), false);
        assert_eq!(first, PointingOutcome::OverlapFirstSeen);
        assert!(!entries[0].self_activated);
        assert!(entries[0].deadline.is_none());
        assert!(entries[0].overlap_warned);

        let second = pointing_step(&mut entries, 0, at(1100), false);
        assert_eq!(second, PointingOutcome::Idle);
    }

    #[test]
    fn pointing_step_extends_deadline_on_repeated_motion_from_holding_entry() {
        let mut entries = [entry_with_layer(Some(1), 3)];
        entries[0].self_activated = true;
        entries[0].deadline = Some(at(200));

        let outcome = pointing_step(&mut entries, 0, at(1000), false);

        assert_eq!(outcome, PointingOutcome::Idle);
        assert!(entries[0].self_activated);
        assert_eq!(entries[0].deadline, Some(at(1000) + entries[0].config.timeout));
    }

    #[test]
    fn pointing_step_resets_overlap_warned_when_we_regain_hold() {
        let mut entries = [entry_with_layer(Some(1), 2)];

        let first = pointing_step(&mut entries, 0, at(1000), false);
        assert_eq!(first, PointingOutcome::OverlapFirstSeen);
        assert!(entries[0].overlap_warned);

        let second = pointing_step(&mut entries, 0, at(1100), true);
        assert_eq!(second, PointingOutcome::Holding);
        assert!(entries[0].self_activated);
        assert!(!entries[0].overlap_warned);
        assert_eq!(entries[0].deadline, Some(at(1100) + entries[0].config.timeout));
    }

    #[test]
    fn timeout_step_releases_multiple_distinct_layers_that_expire_together() {
        let mut entries = [entry_with_layer(Some(1), 3), entry_with_layer(Some(2), 5)];
        entries[0].self_activated = true;
        entries[0].deadline = Some(at(100));
        entries[1].self_activated = true;
        entries[1].deadline = Some(at(150));

        let released = timeout_step(&mut entries, at(200));

        assert_eq!(released.len(), 2);
        assert!(released.contains(&3));
        assert!(released.contains(&5));
        assert!(!entries[0].self_activated);
        assert!(!entries[1].self_activated);
    }

    #[test]
    fn timeout_step_expires_entry_when_deadline_equals_now() {
        let mut entries = [entry_with_layer(Some(1), 3)];
        entries[0].self_activated = true;
        entries[0].deadline = Some(at(100));

        let released = timeout_step(&mut entries, at(100));

        assert_eq!(released.as_slice(), &[3]);
        assert!(!entries[0].self_activated);
    }
}
