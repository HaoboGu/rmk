//! Shared connection type definitions used across RMK crates.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::ble::{BleState, BleStatus};

/// Connection type for the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectionType {
    Usb,
    Ble,
}

/// Unknown stored values default to [`ConnectionType::Usb`] so a downgrade
/// from a newer firmware that wrote an unknown variant falls back to USB
/// rather than refusing to boot.
impl From<u8> for ConnectionType {
    fn from(value: u8) -> Self {
        match value {
            0 => ConnectionType::Usb,
            1 => ConnectionType::Ble,
            _ => ConnectionType::Usb,
        }
    }
}

impl From<ConnectionType> for u8 {
    fn from(value: ConnectionType) -> Self {
        match value {
            ConnectionType::Usb => 0,
            ConnectionType::Ble => 1,
        }
    }
}

/// USB device lifecycle. `Suspended` is distinct from `Configured` because
/// the bus is enumerated but transmission is gated on remote wakeup — the
/// first key still needs to reach the USB writer to trigger that wakeup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Default)]
pub enum UsbState {
    #[default]
    Disabled,
    Enabled,
    Configured,
    Suspended,
}

/// Unified connection status: the single source of truth for transport
/// availability and routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionStatus {
    pub usb: UsbState,
    pub ble: BleStatus,
    /// Derived by [`Self::decide_active`] — never set directly.
    pub active: Option<ConnectionType>,
    /// Tiebreaker when both transports are ready.
    pub preferred: ConnectionType,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self {
            usb: UsbState::Disabled,
            ble: BleStatus::default(),
            active: None,
            preferred: ConnectionType::Usb,
        }
    }
}

impl ConnectionStatus {
    pub fn usb_ready(&self) -> bool {
        matches!(self.usb, UsbState::Configured)
    }

    pub fn ble_ready(&self) -> bool {
        matches!(self.ble.state, BleState::Connected)
    }

    /// Suspended USB still counts here so the first wake key can reach the
    /// USB writer and trigger remote wakeup.
    pub fn any_ready(&self) -> bool {
        self.decide_active().is_some()
    }

    pub fn writable_on(&self, t: ConnectionType) -> bool {
        self.active == Some(t)
    }

    /// Pick the active transport from current readiness + preference. Ready
    /// transports win first; suspended USB stays selected when nothing else
    /// is so remote wakeup remains reachable.
    pub fn decide_active(&self) -> Option<ConnectionType> {
        match (self.usb_ready(), self.ble_ready()) {
            (true, false) => Some(ConnectionType::Usb),
            (false, true) => Some(ConnectionType::Ble),
            (true, true) => Some(self.preferred),
            (false, false) if matches!(self.usb, UsbState::Suspended) => Some(ConnectionType::Usb),
            (false, false) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionStatus, ConnectionType, UsbState};
    use crate::ble::{BleState, BleStatus};

    fn status(usb: UsbState, ble_state: BleState, preferred: ConnectionType) -> ConnectionStatus {
        ConnectionStatus {
            usb,
            ble: BleStatus {
                profile: 0,
                state: ble_state,
            },
            active: None,
            preferred,
        }
    }

    #[test]
    fn decide_active_only_usb_ready() {
        let s = status(UsbState::Configured, BleState::Inactive, ConnectionType::Ble);
        assert_eq!(s.decide_active(), Some(ConnectionType::Usb));
    }

    #[test]
    fn decide_active_only_ble_ready() {
        let s = status(UsbState::Disabled, BleState::Connected, ConnectionType::Usb);
        assert_eq!(s.decide_active(), Some(ConnectionType::Ble));
    }

    #[test]
    fn decide_active_both_ready_prefers_preference() {
        let s = status(UsbState::Configured, BleState::Connected, ConnectionType::Ble);
        assert_eq!(s.decide_active(), Some(ConnectionType::Ble));
        let s = status(UsbState::Configured, BleState::Connected, ConnectionType::Usb);
        assert_eq!(s.decide_active(), Some(ConnectionType::Usb));
    }

    #[test]
    fn decide_active_neither_ready_is_none() {
        let s = status(UsbState::Disabled, BleState::Inactive, ConnectionType::Usb);
        assert_eq!(s.decide_active(), None);
    }

    #[test]
    fn suspended_usb_stays_routable_for_remote_wakeup() {
        let s = status(UsbState::Suspended, BleState::Advertising, ConnectionType::Ble);
        assert_eq!(s.decide_active(), Some(ConnectionType::Usb));
        assert!(s.any_ready());
        assert!(!s.usb_ready());
    }

    #[test]
    fn suspended_usb_yields_to_connected_ble() {
        // Laptop sleep with phone still BLE-connected: cascade picks BLE.
        let mut s = status(UsbState::Suspended, BleState::Connected, ConnectionType::Usb);
        s.active = s.decide_active();
        assert_eq!(s.active, Some(ConnectionType::Ble));
        assert!(!s.usb_ready());
        assert!(s.ble_ready());
    }

    #[test]
    fn writable_on_requires_active_match() {
        let mut s = status(UsbState::Configured, BleState::Connected, ConnectionType::Usb);
        s.active = s.decide_active();
        assert!(s.writable_on(ConnectionType::Usb));
        assert!(!s.writable_on(ConnectionType::Ble));
    }
}
