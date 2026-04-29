use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use embassy_sync::watch::Watch;
use rmk_types::ble::BleState;
#[cfg(test)]
use rmk_types::ble::BleStatus;
use rmk_types::connection::{ConnectionStatus, ConnectionType, UsbState};

use crate::RawMutex;
#[cfg(feature = "_ble")]
use crate::event::BleStatusChangeEvent;
use crate::event::{ConnectionChangeEvent, publish_event};

const CONNECTION_STATUS_RECEIVERS: usize = 2;

/// Override flag for the matrix scan gate.
///
/// Sticky: once set by the BLE sleep-wake path, stays set across subsequent
/// advertise/connect cycles so the user can keep typing through reconnects.
static MATRIX_SCAN_OVERRIDE: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "_ble")]
pub(crate) fn enable_matrix_scan_override() {
    MATRIX_SCAN_OVERRIDE.store(true, Ordering::Release);
}

/// True when the keyboard should keep processing input events.
///
/// This is intentionally broader than "a host transport is writable right
/// now". USB-capable builds keep the input pipeline alive from boot to
/// preserve the old dummy/disconnected path, and the BLE wake override keeps
/// it alive through reconnect windows after advertising timeout.
pub(crate) fn input_processing_ready() -> bool {
    any_transport_ready() || MATRIX_SCAN_OVERRIDE.load(Ordering::Acquire) || cfg!(not(feature = "_no_usb"))
}

/// Single source of truth for transport state and routing. All writes go
/// through the mutator helpers below so the active-output cascade runs and
/// change events fire on every transition.
pub(crate) static CONNECTION_STATUS: Watch<RawMutex, ConnectionStatus, CONNECTION_STATUS_RECEIVERS> =
    Watch::new_with(ConnectionStatus::new());

/// Lock-free mirror of `ConnectionStatus::decide_active`. The HID writer hot
/// path consults this on every report; reading the full `ConnectionStatus`
/// would take a mutex per dispatch.
static ACTIVE_TRANSPORT: AtomicU8 = AtomicU8::new(ACTIVE_NONE);
const ACTIVE_NONE: u8 = 0;
const ACTIVE_USB: u8 = 1;
const ACTIVE_BLE: u8 = 2;

fn encode_active(a: Option<ConnectionType>) -> u8 {
    match a {
        None => ACTIVE_NONE,
        Some(ConnectionType::Usb) => ACTIVE_USB,
        Some(ConnectionType::Ble) => ACTIVE_BLE,
    }
}

pub(crate) fn active_transport() -> Option<ConnectionType> {
    match ACTIVE_TRANSPORT.load(Ordering::Acquire) {
        ACTIVE_USB => Some(ConnectionType::Usb),
        ACTIVE_BLE => Some(ConnectionType::Ble),
        _ => None,
    }
}

pub(crate) fn connection_status() -> ConnectionStatus {
    // `CONNECTION_STATUS` is constructed via `Watch::new_with`, so
    // `try_get` is always `Some`.
    CONNECTION_STATUS
        .try_get()
        .expect("CONNECTION_STATUS initialized via new_with")
}

// Non-atomic read-modify-write: relies on embassy's cooperative scheduling so
// concurrent calls on the same executor can't interleave.
fn update_status(f: impl FnOnce(&mut ConnectionStatus)) {
    let prev = connection_status();
    let mut new = prev;
    f(&mut new);
    if prev == new {
        return;
    }
    let prev_active = prev.decide_active();
    let new_active = new.decide_active();
    if prev_active != new_active {
        ACTIVE_TRANSPORT.store(encode_active(new_active), Ordering::Release);
        if let Some(prev) = prev_active {
            crate::channel::clear_report_channel(prev);
        }
    }
    CONNECTION_STATUS.sender().send(new);

    #[cfg(feature = "_ble")]
    if prev.ble != new.ble {
        publish_event(BleStatusChangeEvent(new.ble));
    }
    if prev.preferred != new.preferred {
        publish_event(ConnectionChangeEvent::new(new.preferred));
    }
}

pub fn set_usb_state(s: UsbState) {
    update_status(|c| c.usb = s);
}

pub(crate) fn set_ble_state(s: BleState) {
    update_status(|c| c.ble.state = s);
}

#[cfg(test)]
pub(crate) fn set_ble_status(s: BleStatus) {
    update_status(|c| c.ble = s);
}

/// Switching profiles always drops the BLE state back to `Inactive`; the
/// connection loop re-advertises and updates state from there.
pub(crate) fn set_ble_profile(profile: u8) {
    update_status(|c| {
        c.ble.profile = profile;
        c.ble.state = BleState::Inactive;
    });
}

/// Persistence is the caller's responsibility — enqueue
/// `FlashOperationMessage::ConnectionType` on `FLASH_CHANNEL`.
pub(crate) fn set_preferred(t: ConnectionType) {
    update_status(|c| c.preferred = t);
}

pub(crate) fn toggle_preferred() -> ConnectionType {
    let mut new = ConnectionType::Usb;
    update_status(|c| {
        c.preferred = match c.preferred {
            ConnectionType::Usb => ConnectionType::Ble,
            ConnectionType::Ble => ConnectionType::Usb,
        };
        new = c.preferred;
    });
    new
}

/// Suspended USB counts here so the first wake key can reach the USB writer
/// and trigger remote wakeup.
pub(crate) fn any_transport_ready() -> bool {
    active_transport().is_some()
}

pub(crate) fn writable_on(t: ConnectionType) -> bool {
    active_transport() == Some(t)
}

#[cfg(not(feature = "_no_usb"))]
pub(crate) fn usb_suspended() -> bool {
    connection_status().usb == UsbState::Suspended
}

#[cfg(feature = "_ble")]
pub(crate) fn current_profile() -> u8 {
    connection_status().ble.profile
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::Ordering;
    use std::sync::{Mutex, OnceLock};

    use embassy_futures::select::{Either, select};
    use embassy_time::{Duration, Timer};

    use super::{
        CONNECTION_STATUS, ConnectionStatus, ConnectionType, MATRIX_SCAN_OVERRIDE, UsbState, input_processing_ready,
        set_preferred, set_usb_state,
    };
    use crate::event::{ConnectionChangeEvent, EventSubscriber, SubscribableEvent};
    use crate::test_support::test_block_on as block_on;

    fn state_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_state() {
        let initial = ConnectionStatus::default();
        super::ACTIVE_TRANSPORT.store(super::encode_active(initial.decide_active()), Ordering::Release);
        CONNECTION_STATUS.sender().send(initial);
        MATRIX_SCAN_OVERRIDE.store(false, Ordering::Release);
    }

    #[cfg(not(feature = "_no_usb"))]
    #[test]
    fn usb_builds_keep_input_processing_active_without_transport() {
        let _guard = state_test_lock().lock().unwrap();
        reset_state();

        assert!(input_processing_ready());
    }

    #[test]
    fn override_keeps_input_processing_active_without_transport() {
        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        MATRIX_SCAN_OVERRIDE.store(true, Ordering::Release);

        assert!(input_processing_ready());
    }

    #[test]
    fn preferred_transport_change_publishes_connection_event() {
        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        set_usb_state(UsbState::Configured);
        let mut sub = ConnectionChangeEvent::subscriber();

        set_preferred(ConnectionType::Ble);

        let event = block_on(sub.next_event());
        assert_eq!(event.0, ConnectionType::Ble);
    }

    #[test]
    fn active_transport_flip_does_not_publish_connection_event() {
        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        let mut sub = ConnectionChangeEvent::subscriber();

        set_usb_state(UsbState::Configured);

        match block_on(select(Timer::after(Duration::from_millis(1)), sub.next_event())) {
            Either::First(_) => {}
            Either::Second(event) => panic!("unexpected connection change event: {:?}", event),
        }
    }

    #[cfg(not(feature = "_no_usb"))]
    #[test]
    fn flipping_away_from_active_clears_its_report_channel() {
        use crate::channel::USB_REPORT_CHANNEL;
        use crate::hid::{KeyboardReport, Report};

        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        set_usb_state(UsbState::Configured);
        assert_eq!(super::active_transport(), Some(ConnectionType::Usb));

        // Drain anything left over from earlier tests, then queue a sentinel
        // that would otherwise persist across a flip.
        USB_REPORT_CHANNEL.clear();
        USB_REPORT_CHANNEL
            .try_send(Report::KeyboardReport(KeyboardReport::default()))
            .expect("channel should have capacity for sentinel");
        assert!(USB_REPORT_CHANNEL.try_receive().is_ok());
        USB_REPORT_CHANNEL
            .try_send(Report::KeyboardReport(KeyboardReport::default()))
            .expect("channel should have capacity for sentinel");

        set_usb_state(UsbState::Disabled);
        assert!(super::active_transport().is_none());
        assert!(
            USB_REPORT_CHANNEL.try_receive().is_err(),
            "USB_REPORT_CHANNEL should be drained when USB stops being active"
        );
    }
}
