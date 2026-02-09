//! Controller events
//!
//! This module contains event types for keyboard state changes, LED indicators,
//! connection status, and other system-level events.

#[cfg(feature = "_ble")]
mod ble;
mod connection;
mod input;
mod keyboard_state;
mod power;
#[cfg(feature = "split")]
mod split;

#[cfg(feature = "_ble")]
pub use ble::{BleProfileChangeEvent, BleStateChangeEvent};
pub use connection::{ConnectionChangeEvent, ConnectionType};
pub use input::{KeyEvent, ModifierEvent};
pub use keyboard_state::{LayerChangeEvent, LedIndicatorEvent, SleepStateEvent, WpmUpdateEvent};
#[cfg(feature = "_ble")]
pub use power::BatteryStateEvent;
#[cfg(feature = "split")]
pub use split::{CentralConnectedEvent, PeripheralConnectedEvent};
#[cfg(all(feature = "split", feature = "_ble"))]
pub use split::{ClearPeerEvent, PeripheralBatteryEvent};
