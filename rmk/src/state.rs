use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::blocking_mutex::Mutex;
use rmk_types::ble::BleState;
#[cfg(feature = "_ble")]
use rmk_types::ble::BleStatus;
use rmk_types::connection::{ConnectionStatus, ConnectionType, UsbState};

use crate::RawMutex;
#[cfg(feature = "_ble")]
use crate::event::BleStatusChangeEvent;
use crate::event::{ConnectionChangeEvent, publish_event};

/// Override flag for the matrix scan gate.
///
/// Set by the BLE advertising-timeout path so the matrix keeps scanning
/// during the reconnect window. Cleared automatically once a BLE connection
/// is re-established
static MATRIX_SCAN_OVERRIDE: AtomicBool = AtomicBool::new(false);

/// Single source of truth for transport state and routing. All writes go
/// through the mutator helpers below so the active-output cascade runs and
/// change events fire on every transition.
pub(crate) static CONNECTION_STATUS: Mutex<RawMutex, Cell<ConnectionStatus>> =
    Mutex::new(Cell::new(ConnectionStatus::new()));

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
    active_transport().is_some() || MATRIX_SCAN_OVERRIDE.load(Ordering::Acquire) || cfg!(not(feature = "_no_usb"))
}

pub(crate) fn active_transport() -> Option<ConnectionType> {
    CONNECTION_STATUS.lock(|c| c.get().decide_active())
}

pub(crate) fn current_usb_state() -> UsbState {
    CONNECTION_STATUS.lock(|c| c.get().usb)
}

#[cfg(feature = "_ble")]
pub(crate) fn current_ble_status() -> BleStatus {
    CONNECTION_STATUS.lock(|c| c.get().ble)
}

/// Read-modify-write the connection status atomically.
fn update_status(f: impl FnOnce(&mut ConnectionStatus)) {
    let Some((prev, new)) = CONNECTION_STATUS.lock(|c| {
        let prev = c.get();
        let mut new = prev;
        f(&mut new);
        if prev == new {
            return None;
        }
        c.set(new);
        Some((prev, new))
    }) else {
        return;
    };

    let prev_active = prev.decide_active();
    let new_active = new.decide_active();

    if prev_active != new_active
        && let Some(prev_active) = prev_active
    {
        // Drain after the commit so any producer racing past the mutex reads
        // the new state and routes to the new channel rather than the one
        // about to be cleared.
        crate::channel::clear_and_release_report_channel(prev_active);
    }

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
    // Reaching Connected means the reconnect window we set the override for
    // is over. The next advertising-timeout cycle will set it again if needed.
    if s == BleState::Connected {
        MATRIX_SCAN_OVERRIDE.store(false, Ordering::Release);
    }
    update_status(|c| c.ble.state = s);
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
#[cfg(feature = "_ble")]
pub(crate) async fn load_preferred_connection() -> ConnectionType {
    #[cfg(feature = "storage")]
    let stored = crate::storage::read_connection_type().await;
    #[cfg(not(feature = "storage"))]
    let stored: Option<ConnectionType> = None;
    match stored {
        Some(c) => c,
        #[cfg(feature = "_no_usb")]
        None => ConnectionType::Ble,
        #[cfg(not(feature = "_no_usb"))]
        None => ConnectionType::Usb,
    }
}

#[cfg(all(feature = "_ble", not(feature = "_no_usb")))]
pub(crate) async fn toggle_preferred() {
    let mut new = ConnectionType::Usb;
    update_status(|c| {
        c.preferred = match c.preferred {
            ConnectionType::Usb => ConnectionType::Ble,
            ConnectionType::Ble => ConnectionType::Usb,
        };
        new = c.preferred;
    });
    info!("Switching preferred transport to: {:?}", new);
    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL
        .send(crate::storage::FlashOperationMessage::ConnectionType(new))
        .await;
}

#[cfg(feature = "_ble")]
pub(crate) fn current_profile() -> u8 {
    CONNECTION_STATUS.lock(|c| c.get().ble.profile)
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::Ordering;
    use std::sync::{Mutex, OnceLock};

    use embassy_futures::select::{Either, select};
    use embassy_time::{Duration, Timer};
    use rmk_types::ble::BleState;

    use super::{
        CONNECTION_STATUS, ConnectionStatus, ConnectionType, MATRIX_SCAN_OVERRIDE, UsbState, input_processing_ready,
        set_ble_state, set_preferred_connection, set_usb_state,
    };
    use crate::event::{ConnectionChangeEvent, EventSubscriber, SubscribableEvent};
    use crate::hid::{KeyboardReport, Report};
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

    fn pressed_keyboard_report() -> Report {
        Report::KeyboardReport(KeyboardReport {
            modifier: 0x02,
            reserved: 0,
            leds: 0,
            keycodes: [4, 0, 0, 0, 0, 0],
        })
    }

    fn assert_all_up_keyboard_report(report: Report) {
        match report {
            Report::KeyboardReport(r) => {
                assert_eq!(r.modifier, 0);
                assert_eq!(r.reserved, 0);
                assert_eq!(r.leds, 0);
                assert_eq!(r.keycodes, [0; 6]);
            }
            _ => panic!("expected keyboard all-up report"),
        }
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
    fn flipping_away_from_active_clears_stale_reports_and_queues_all_up() {
        use crate::channel::USB_REPORT_CHANNEL;

        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        set_usb_state(UsbState::Configured);
        assert_eq!(super::active_transport(), Some(ConnectionType::Usb));

        // Drain anything left over from earlier tests, then queue a sentinel
        // that would otherwise persist across a flip.
        USB_REPORT_CHANNEL.clear();
        USB_REPORT_CHANNEL
            .try_send(pressed_keyboard_report())
            .expect("channel should have capacity for sentinel");
        assert!(USB_REPORT_CHANNEL.try_receive().is_ok());
        USB_REPORT_CHANNEL
            .try_send(pressed_keyboard_report())
            .expect("channel should have capacity for sentinel");

        set_usb_state(UsbState::Disabled);
        assert!(super::active_transport().is_none());
        assert_all_up_keyboard_report(
            USB_REPORT_CHANNEL
                .try_receive()
                .expect("USB_REPORT_CHANNEL should contain keyboard all-up report"),
        );
        assert!(
            USB_REPORT_CHANNEL.try_receive().is_err(),
            "USB_REPORT_CHANNEL should contain only the all-up report"
        );
    }

    #[cfg(not(feature = "_no_usb"))]
    #[test]
    fn blocked_send_drops_report_after_transport_change() {
        use embassy_futures::join::join;

        use crate::channel::{USB_REPORT_CHANNEL, send_hid_report};

        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        set_usb_state(UsbState::Configured);

        for _ in 0..crate::REPORT_CHANNEL_SIZE {
            USB_REPORT_CHANNEL
                .try_send(pressed_keyboard_report())
                .expect("channel should have capacity while filling");
        }

        block_on(join(
            send_hid_report(Report::KeyboardReport(KeyboardReport::default())),
            async {
                Timer::after(Duration::from_millis(1)).await;
                set_usb_state(UsbState::Disabled);
            },
        ));

        assert_all_up_keyboard_report(
            USB_REPORT_CHANNEL
                .try_receive()
                .expect("USB_REPORT_CHANNEL should contain keyboard all-up report"),
        );
        assert!(
            USB_REPORT_CHANNEL.try_receive().is_err(),
            "USB_REPORT_CHANNEL should contain only the all-up report"
        );
    }

    #[cfg(all(not(feature = "_no_usb"), feature = "_ble"))]
    #[test]
    fn usb_preference_flip_releases_previous_ble_transport() {
        use crate::channel::BLE_REPORT_CHANNEL;

        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        set_preferred_connection(ConnectionType::Usb);
        set_ble_state(BleState::Connected);
        assert_eq!(super::active_transport(), Some(ConnectionType::Ble));

        BLE_REPORT_CHANNEL
            .try_send(pressed_keyboard_report())
            .expect("BLE report channel should have capacity for sentinel");

        set_usb_state(UsbState::Configured);
        assert_eq!(super::active_transport(), Some(ConnectionType::Usb));
        assert_all_up_keyboard_report(
            BLE_REPORT_CHANNEL
                .try_receive()
                .expect("BLE_REPORT_CHANNEL should contain keyboard all-up report"),
        );
        assert!(
            BLE_REPORT_CHANNEL.try_receive().is_err(),
            "BLE_REPORT_CHANNEL should contain only the all-up report"
        );
    }

    #[test]
    fn ble_connected_clears_matrix_scan_override() {
        let _guard = state_test_lock().lock().unwrap();
        reset_state();
        MATRIX_SCAN_OVERRIDE.store(true, Ordering::Release);

        set_ble_state(BleState::Advertising);
        assert!(
            MATRIX_SCAN_OVERRIDE.load(Ordering::Acquire),
            "Advertising should not clear the override"
        );

        set_ble_state(BleState::Connected);
        assert!(
            !MATRIX_SCAN_OVERRIDE.load(Ordering::Acquire),
            "Connected should clear the override"
        );
    }

    #[cfg(not(feature = "_no_usb"))]
    #[test]
    fn blocked_send_enqueues_when_transport_stays_active() {
        use embassy_futures::join::join;

        use crate::channel::{USB_REPORT_CHANNEL, send_hid_report};

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
