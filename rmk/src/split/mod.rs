use core::sync::atomic::AtomicBool;

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

#[cfg(feature = "_ble")]
use crate::event::BatteryStatusEvent;
use crate::event::{KeyboardEvent, PointingEvent};

/// Mirror of the central's `active_transport().is_some()` on the peripheral
/// MCU. The central syncs this over the split link via `SplitMessage::ConnectionState`
/// so peripheral-side consumers (e.g. a status display) can show whether the
/// central currently has a host transport. Read-only signal — nothing in the
/// input pipeline gates on it.
pub(crate) static CENTRAL_HOST_CONNECTED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "_ble")]
pub mod ble;
pub mod central;
/// Common abstraction layer of split driver
pub(crate) mod driver;
pub mod peripheral;
#[cfg(feature = "rp2040")]
pub mod rp;
#[cfg(not(feature = "_ble"))]
pub mod serial;

/// Maximum size of a split message
pub const SPLIT_MESSAGE_MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE + 4;

/// Message used from central & peripheral communication
#[repr(u8)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum SplitMessage {
    /// Keyboard event, from peripheral to central
    Key(KeyboardEvent),
    /// Pointing device event, from peripheral to central
    Pointing(PointingEvent),
    /// Led state, on/off, from central to peripheral
    LedState(bool),
    /// `true` when the central has an active host transport (USB Configured/
    /// Suspended or BLE Connected). Synced central→peripheral periodically and
    /// on change. Informational only — nothing in the input pipeline gates on
    /// this; consumers are peripheral-side display/status code.
    ConnectionState(bool),
    /// BLE Address, used in syncing address between central and peripheral
    Address([u8; 6]),
    /// Clear the saved peer info
    ClearPeer,
    /// Lock state led indicator from central to peripheral
    KeyboardIndicator(u8),
    /// Layer number from central to peripheral
    Layer(u8),
    /// WPM from central to peripheral
    #[cfg(feature = "display")]
    Wpm(u16),
    /// Modifier state from central to peripheral
    #[cfg(feature = "display")]
    Modifier(u8),
    /// Sleep state from central to peripheral
    #[cfg(feature = "display")]
    SleepState(bool),
    /// Battery status, from peripheral to central
    #[cfg(feature = "_ble")]
    BatteryStatus(BatteryStatusEvent),
}
