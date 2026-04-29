use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::blocking_mutex::Mutex;
use rmk_types::ble::BleState;
#[cfg(test)]
use rmk_types::ble::BleStatus;
use rmk_types::connection::{ConnectionStatus, ConnectionType, UsbState};

use crate::RawMutex;
#[cfg(feature = "_ble")]
use crate::event::BleStatusChangeEvent;
use crate::event::{ConnectionChangeEvent, publish_event};

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
pub(crate) static CONNECTION_STATUS: Mutex<RawMutex, Cell<ConnectionStatus>> =
    Mutex::new(Cell::new(ConnectionStatus::new()));

pub(crate) fn active_transport() -> Option<ConnectionType> {
    connection_status().decide_active()
}

pub(crate) fn connection_status() -> ConnectionStatus {
    CONNECTION_STATUS.lock(|c| c.get())
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
    if prev_active != new.decide_active() && let Some(prev) = prev_active {
        crate::channel::clear_report_channel(prev);
    }
    CONNECTION_STATUS.lock(|c| c.set(new));

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
pub(crate) fn set_preferred_connection(t: ConnectionType) {
    update_status(|c| c.preferred = t);
}

/// Load the preferred connection type at startup.
///
/// With the `storage` feature, reads the persisted `ConnectionType` from flash;
/// otherwise falls back to a build-time default — `Ble` when USB is disabled, `Usb` otherwise.
pub(crate) async fn load_preferred_connection() -> ConnectionType {
    #[cfg(feature = "storage")]
    let stored = crate::storage::read_setting(crate::storage::StorageKey::ConnectionType).await;
    #[cfg(not(feature = "storage"))]
    let stored: Option<u8> = None;
    match stored {
        Some(c) => c.into(),
        #[cfg(feature = "_no_usb")]
        None => ConnectionType::Ble,
        #[cfg(not(feature = "_no_usb"))]
        None => ConnectionType::Usb,
    }
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
        set_preferred_connection, set_usb_state,
    };
    use crate::event::{ConnectionChangeEvent, EventSubscriber, SubscribableEvent};
    use crate::test_support::test_block_on as block_on;

    fn state_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_state() {
        CONNECTION_STATUS.lock(|c| c.set(ConnectionStatus::default()));
        MATRIX_SCAN_OVERRIDE.store(false, Ordering::Release);
        #[cfg(not(feature = "_no_usb"))]
        crate::channel::USB_REPORT_CHANNEL.clear();
        #[cfg(feature = "_ble")]
        crate::channel::BLE_REPORT_CHANNEL.clear();
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

        set_preferred_connection(ConnectionType::Ble);

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

    #[cfg(not(feature = "_no_usb"))]
    #[test]
    fn blocked_send_drops_report_after_transport_change() {
        use embassy_futures::join::join;

        use crate::channel::{USB_REPORT_CHANNEL, send_hid_report};
        use crate::hid::{KeyboardReport, Report};

        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        set_usb_state(UsbState::Configured);

        for _ in 0..crate::REPORT_CHANNEL_SIZE {
            USB_REPORT_CHANNEL
                .try_send(Report::KeyboardReport(KeyboardReport::default()))
                .expect("channel should have capacity while filling");
        }

        block_on(join(
            send_hid_report(Report::KeyboardReport(KeyboardReport::default())),
            async {
                Timer::after(Duration::from_millis(1)).await;
                set_usb_state(UsbState::Disabled);
            },
        ));

        assert!(USB_REPORT_CHANNEL.try_receive().is_err());
    }

    #[cfg(not(feature = "_no_usb"))]
    #[test]
    fn blocked_send_enqueues_when_transport_stays_active() {
        use embassy_futures::join::join;

        use crate::channel::{USB_REPORT_CHANNEL, send_hid_report};
        use crate::hid::{KeyboardReport, Report};

        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        set_usb_state(UsbState::Configured);

        for _ in 0..crate::REPORT_CHANNEL_SIZE {
            USB_REPORT_CHANNEL
                .try_send(Report::KeyboardReport(KeyboardReport::default()))
                .expect("channel should have capacity while filling");
        }

        block_on(join(
            send_hid_report(Report::KeyboardReport(KeyboardReport::default())),
            async {
                Timer::after(Duration::from_millis(1)).await;
                let _ = USB_REPORT_CHANNEL.try_receive();
            },
        ));

        assert_eq!(USB_REPORT_CHANNEL.len(), crate::REPORT_CHANNEL_SIZE);
    }
}
