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

use embassy_futures::select::{Either, Either3, select, select3};
use embassy_time::{Instant, Timer};
use heapless::Vec;

use crate::AUTO_MOUSE_LAYER_MAX_NUM;
use crate::config::AutoMouseLayerConfig;
use crate::core_traits::Runnable;
use crate::event::{Axis, AxisValType, EventSubscriber, LayerChangeEvent, PointingEvent, SubscribableEvent};
use crate::keymap::KeyMap;

/// [`Runnable`] for the auto mouse layer task.
///
/// Construct with [`run_auto_mouse_layer_if_enabled`] and pass to `run_all!`.
/// If the keymap has no auto mouse layer configured (or every entry's layer is
/// out of range), [`Runnable::run`] parks forever on [`core::future::pending`]
/// so it can sit alongside the other tasks without doing anything.
pub struct AutoMouseLayerRunner<'a, 'k> {
    keymap: &'a KeyMap<'k>,
}

/// Build a [`Runnable`] that activates the configured auto mouse layer on
/// pointer motion. Pass the returned value to `run_all!`.
pub fn run_auto_mouse_layer_if_enabled<'a, 'k>(keymap: &'a KeyMap<'k>) -> AutoMouseLayerRunner<'a, 'k> {
    AutoMouseLayerRunner { keymap }
}

/// Per-entry runtime state.
#[derive(Clone, Copy)]
struct EntryState {
    config: AutoMouseLayerConfig,
    /// `true` while this entry is the one holding the layer on.
    self_activated: bool,
    /// Set when the entry is self-activated; the next tick fires at this point
    /// to release ownership if no motion has refreshed it.
    deadline: Option<Instant>,
    /// Whether we have already warned about the entry's layer overlapping a
    /// manually-activated layer.
    overlap_warned: bool,
}

impl Runnable for AutoMouseLayerRunner<'_, '_> {
    async fn run(&mut self) -> ! {
        let keymap = self.keymap;
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

        if entries.is_empty() {
            core::future::pending().await
        }

        let mut subscriber = PointingEvent::subscriber();
        let mut layer_sub = LayerChangeEvent::subscriber();

        loop {
            let earliest = earliest_deadline(&entries);
            let next = match earliest {
                Some(deadline) => {
                    match select3(Timer::at(deadline), subscriber.next_event(), layer_sub.next_event()).await {
                        Either3::First(_) => Tick::Timeout,
                        Either3::Second(e) => Tick::Pointing(e),
                        Either3::Third(e) => Tick::Layer(e),
                    }
                }
                None => match select(subscriber.next_event(), layer_sub.next_event()).await {
                    Either::First(e) => Tick::Pointing(e),
                    Either::Second(e) => Tick::Layer(e),
                },
            };

            match next {
                Tick::Timeout => {
                    let now = Instant::now();
                    for entry in entries.iter_mut() {
                        if entry.self_activated && entry.deadline.is_some_and(|d| d <= now) {
                            keymap.deactivate_layer_if_active(entry.config.target_layer);
                            entry.self_activated = false;
                            entry.deadline = None;
                        }
                    }
                }
                Tick::Pointing(event) => {
                    let Some(idx) = match_entry(&entries, event.device_id) else {
                        continue;
                    };
                    let entry = &mut entries[idx];
                    if !is_cursor_motion(&event, entry.config.threshold) {
                        continue;
                    }
                    if keymap.activate_layer_if_inactive(entry.config.target_layer) {
                        entry.self_activated = true;
                        entry.overlap_warned = false;
                    } else if !entry.self_activated && !entry.overlap_warned {
                        warn!(
                            "auto_mouse_layer: layer {} is already active when motion was detected; \
                             the layer is likely driven by another key (MO/TG). The auto mouse layer \
                             will not be deactivated on timeout while overlap holds.",
                            entry.config.target_layer
                        );
                        entry.overlap_warned = true;
                    }
                    if entry.self_activated {
                        entry.deadline = Some(Instant::now() + entry.config.timeout);
                    }
                }
                Tick::Layer(LayerChangeEvent(top)) => {
                    for entry in entries.iter_mut() {
                        // Drop ownership if a keyboard-driven change turned our layer off,
                        // so the next motion can re-acquire it cleanly.
                        if entry.self_activated && !keymap.is_layer_active(entry.config.target_layer) {
                            entry.self_activated = false;
                            entry.deadline = None;
                            trace!(
                                "auto_mouse_layer: released layer {} (top now {})",
                                entry.config.target_layer, top
                            );
                        }
                    }
                }
            }
        }
    }
}

enum Tick {
    Timeout,
    Pointing(PointingEvent),
    Layer(LayerChangeEvent),
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

    #[test]
    fn event_for_carries_device_id() {
        // Smoke test confirming the test helper does what later integration
        // tests would rely on.
        let zero = axis(Axis::Z, 0);
        let e = event_for(3, [axis(Axis::X, 5), axis(Axis::Y, 0), zero]);
        assert_eq!(e.device_id, 3);
        assert!(is_cursor_motion(&e, 1));
    }
}
