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
pub use connection::{ConnectionChangedEvent, ConnectionType};
pub use input::{KeyEvent, ModifierEvent};
pub use keyboard_state::{LayerChangeEvent, LedIndicatorEvent, SleepStateEvent, WpmUpdateEvent};
#[cfg(feature = "_ble")]
pub use power::{BatteryLevelEvent, ChargingStateEvent};
#[cfg(all(feature = "split", feature = "_ble"))]
pub use split::ClearPeerEvent;
#[cfg(feature = "split")]
pub use split::{CentralConnectedEvent, PeripheralBatteryEvent, PeripheralConnectedEvent};
