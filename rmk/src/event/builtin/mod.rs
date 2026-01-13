//! Built-in controller events

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
pub use keyboard_state::{LayerChangeEvent, LedIndicatorEvent, WpmUpdateEvent};
#[cfg(feature = "_ble")]
pub use power::{BatteryLevelEvent, ChargingStateEvent, SleepStateEvent};
#[cfg(all(feature = "split", feature = "_ble"))]
pub use split::ClearPeerEvent;
#[cfg(feature = "split")]
pub use split::{CentralConnectedEvent, PeripheralBatteryEvent, PeripheralConnectedEvent};
