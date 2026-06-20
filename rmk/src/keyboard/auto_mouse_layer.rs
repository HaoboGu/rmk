//! Auto mouse layer
//!
//! Subscribes to [`PointingEvent`]s coming from pointing devices (e.g. PMW3610)
//! and activates a configured layer whenever cursor motion (X/Y axis) is
//! detected.

use embassy_futures::select::{Either, select};
use embassy_time::{Instant, Timer};

use crate::event::{Axis, AxisValType, EventSubscriber, PointingEvent, SubscribableEvent};
use crate::keymap::KeyMap;

/// Run the auto mouse layer task if it is enabled in the keymap's behavior
/// configuration.
pub async fn run_auto_mouse_layer_if_enabled(keymap: &KeyMap<'_>) {
    let Some(config) = keymap.auto_mouse_layer_config() else {
        return;
    };
    let num_layer = keymap.num_layer();
    if (config.layer as usize) >= num_layer {
        warn!(
            "auto_mouse_layer: configured layer {} is out of range (keymap has {} layers); \
             auto mouse layer task will not run",
            config.layer, num_layer
        );
        return;
    }
    let threshold = config.threshold.max(1);

    let mut subscriber = PointingEvent::subscriber();
    // Tracks whether this task is the one that turned the layer on.
    let mut self_activated = false;
    // Whether we have already warned about the auto mouse layer overlapping a
    // manually-activated layer.
    let mut overlap_warned = false;

    loop {
        // Wait for the next pointing event and filter for cursor motion
        let event = subscriber.next_event().await;
        if !is_cursor_motion(&event, threshold) {
            continue;
        }

        if keymap.activate_layer_if_inactive(config.layer) {
            self_activated = true;
            overlap_warned = false;
        } else if !overlap_warned {
            warn!(
                "auto_mouse_layer: layer {} is already active when motion was detected; \
                 the layer is likely driven by another key (MO/TG). The auto mouse layer \
                 will not be deactivated on timeout while overlap holds.",
                config.layer
            );
            overlap_warned = true;
        }

        // Stay active as long as motion keeps arriving within the timeout
        // window. Each motion event refreshes the deadline; non-motion events
        // are ignored without extending it.
        let mut deadline = Instant::now() + config.timeout;
        loop {
            match select(Timer::at(deadline), subscriber.next_event()).await {
                Either::First(_) => {
                    if self_activated {
                        keymap.deactivate_layer_if_active(config.layer);
                        self_activated = false;
                    }
                    break;
                }
                Either::Second(next) => {
                    if !is_cursor_motion(&next, threshold) {
                        continue;
                    }
                    deadline = Instant::now() + config.timeout;
                    if keymap.activate_layer_if_inactive(config.layer) {
                        self_activated = true;
                        overlap_warned = false;
                    }
                }
            }
        }
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
        PointingEvent { device_id: 0, axes }
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
}
