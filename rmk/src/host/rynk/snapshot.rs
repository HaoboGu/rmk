//! Latest-value cache for the three pure event-stream topics
//! (`WpmUpdate`, `SleepState`, `LedIndicator`).
//!
//! These topics have no producer-side persistent store, so a host that
//! just connected can't know the current value until the next change
//! fires. The service runs [`run_topic_snapshot`] alongside the
//! transports — it subscribes to all three events and latches each
//! payload into a static atomic. The matching `Get*` handlers read
//! the atomic directly.
//!
//! `Ordering::Relaxed` is fine on both sides: there's no cross-field
//! invariant to preserve and each field is independently consumed.
//!
//! Subscribers consume one slot per event from the PubSub channel — the
//! `event_default.toml` entries reserve that slot.
//!
//! The pre-init value (`0` / `false` / empty `LedIndicator`) is what
//! the host sees before the first event fires. Hosts that need to know
//! "no value yet" should rely on the topic push for the first update.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, Ordering};

use futures::FutureExt;
use rmk_types::led_indicator::LedIndicator;

use crate::event::{EventSubscriber, LedIndicatorEvent, SleepStateEvent, SubscribableEvent, WpmUpdateEvent};

static LATEST_WPM: AtomicU16 = AtomicU16::new(0);
static LATEST_SLEEP: AtomicBool = AtomicBool::new(false);
static LATEST_LED: AtomicU8 = AtomicU8::new(0);

pub(in crate::host::rynk) fn wpm() -> u16 {
    LATEST_WPM.load(Ordering::Relaxed)
}

pub(in crate::host::rynk) fn sleep_state() -> bool {
    LATEST_SLEEP.load(Ordering::Relaxed)
}

pub(in crate::host::rynk) fn led_indicator() -> LedIndicator {
    LedIndicator::from_bits(LATEST_LED.load(Ordering::Relaxed))
}

/// Drives the topic snapshot cache. Subscribes to `WpmUpdateEvent`,
/// `SleepStateEvent`, and `LedIndicatorEvent`, and writes each fresh
/// payload into the matching atomic. Join into `run_all!()` once per
/// firmware — multiple instances would race on the same atomics.
pub async fn run_topic_snapshot() -> ! {
    let mut wpm_sub = WpmUpdateEvent::subscriber();
    let mut sleep_sub = SleepStateEvent::subscriber();
    let mut led_sub = LedIndicatorEvent::subscriber();
    loop {
        futures::select_biased! {
            e = wpm_sub.next_event().fuse() => LATEST_WPM.store(e.0, Ordering::Relaxed),
            e = sleep_sub.next_event().fuse() => LATEST_SLEEP.store(e.0, Ordering::Relaxed),
            e = led_sub.next_event().fuse() => LATEST_LED.store(e.0.into_bits(), Ordering::Relaxed),
        }
    }
}
