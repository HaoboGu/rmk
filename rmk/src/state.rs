use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::watch::Watch;
use rmk_types::ble::{BleState, BleStatus};
pub use rmk_types::connection::{ConnectionStatus, ConnectionType, UsbState};

use crate::RawMutex;
#[cfg(feature = "_ble")]
use crate::event::BleStatusChangeEvent;
use crate::event::{ConnectionChangeEvent, publish_event};

const CONNECTION_STATUS_RECEIVERS: usize = 2;

/// Override flag for the matrix scan gate.
///
/// Sticky: once set by the BLE sleep-wake path, stays set across subsequent
/// advertise/connect cycles so the user can keep typing through reconnects.
pub(crate) static MATRIX_SCAN_OVERRIDE: AtomicBool = AtomicBool::new(false);

/// True when the keyboard should keep processing input events.
///
/// This is intentionally broader than "a host transport is writable right
/// now". USB-capable builds keep the input pipeline alive from boot to
/// preserve the old dummy/disconnected path, and the BLE wake override keeps
/// it alive through reconnect windows after advertising timeout.
pub fn input_processing_ready() -> bool {
    any_transport_ready() || MATRIX_SCAN_OVERRIDE.load(Ordering::Acquire) || cfg!(not(feature = "_no_usb"))
}

/// Single source of truth for transport state and routing. All writes go
/// through the mutator helpers below so the active-output cascade runs and
/// change events fire on every transition.
pub(crate) static CONNECTION_STATUS: Watch<RawMutex, ConnectionStatus, CONNECTION_STATUS_RECEIVERS> = Watch::new();

pub fn connection_status() -> ConnectionStatus {
    CONNECTION_STATUS.try_get().unwrap_or_default()
}

// Non-atomic read-modify-write: relies on embassy's cooperative scheduling so
// concurrent calls on the same executor can't interleave.
fn update_status(f: impl FnOnce(&mut ConnectionStatus)) {
    let prev = connection_status();
    let mut new = prev;
    f(&mut new);
    new.active = new.decide_active();
    if prev == new {
        return;
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

pub fn set_ble_state(s: BleState) {
    update_status(|c| c.ble.state = s);
}

pub fn set_ble_status(s: BleStatus) {
    update_status(|c| c.ble = s);
}

/// Switching profiles always drops the BLE state back to `Inactive`; the
/// connection loop re-advertises and updates state from there.
pub fn set_ble_profile(profile: u8) {
    update_status(|c| {
        c.ble.profile = profile;
        c.ble.state = BleState::Inactive;
    });
}

/// Persistence is the caller's responsibility — enqueue
/// `FlashOperationMessage::ConnectionType` on `FLASH_CHANNEL`.
pub fn set_preferred(t: ConnectionType) {
    update_status(|c| c.preferred = t);
}

pub fn toggle_preferred() -> ConnectionType {
    let new = match connection_status().preferred {
        ConnectionType::Usb => ConnectionType::Ble,
        ConnectionType::Ble => ConnectionType::Usb,
    };
    update_status(|c| c.preferred = new);
    new
}

/// Suspended USB counts here so the first wake key can reach the USB writer
/// and trigger remote wakeup.
pub fn any_transport_ready() -> bool {
    connection_status().any_ready()
}

pub fn writable_on(t: ConnectionType) -> bool {
    connection_status().writable_on(t)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use embassy_futures::select::{Either, select};
    use embassy_time::{Duration, Timer};

    use super::{
        CONNECTION_STATUS, ConnectionStatus, ConnectionType, MATRIX_SCAN_OVERRIDE, UsbState, input_processing_ready,
        set_preferred, set_usb_state,
    };
    use core::sync::atomic::Ordering;
    use crate::event::{ConnectionChangeEvent, EventSubscriber, SubscribableEvent};
    use crate::test_support::test_block_on as block_on;

    fn state_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_state() {
        CONNECTION_STATUS.sender().send(ConnectionStatus::default());
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
}
