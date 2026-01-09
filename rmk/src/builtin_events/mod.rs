//! # Built-in Controller Events
//!
//! This module contains all built-in event types for the RMK controller system.
//! Each event type implements the `ControllerEventTrait` and has its own dedicated channel.
//!
//! ## All Events
//!
//! ### KeyboardInputEvent
//! - `Key { keyboard_event, key_action }` - Key press/release with action
//! - `Modifier(ModifierCombination)` - Modifier keys state changed
//!
//! ### PowerEvent
//! - `Battery(u8)` - Battery level changed (0-100)
//! - `Charging(bool)` - Charging state changed
//! - `Sleep(bool)` - Sleep/wake state changed
//!
//! ### KeyboardStateEvent
//! - `Layer(u8)` - Active layer changed
//! - `Wpm(u16)` - Words per minute updated
//! - `Indicator(LedIndicator)` - LED indicator state changed
//!
//! ### ConnectionEvent
//! - `Type(ConnectionType)` - Connection type changed (USB/BLE)
//! - `BleState { profile, state }` - BLE profile state changed
//! - `BleProfile(u8)` - Active BLE profile changed
//!
//! ### SplitEvent
//! - `PeripheralConnected { id, connected }` - Split peripheral connection state
//! - `PeripheralBattery { id, level }` - Split peripheral battery level
//! - `CentralConnected(bool)` - Split central connection state (from peripheral's perspective)
//! - `ClearPeer` - Clear BLE peer information signal
//!
//! ## Usage
//!
//! ### Publishing Events
//!
//! ```ignore
//! use rmk::builtin_events::PowerEvent;
//! use rmk::event::publish;
//!
//! // Publish a battery level change - using helper function
//! publish(PowerEvent::battery(85));
//!
//! // Or use the variant directly
//! publish(PowerEvent::Battery(85));
//! ```
//!
//! ### Subscribing to Events
//!
//! Controllers can subscribe to events using the `#[controller]` macro:
//!
//! ```ignore
//! use rmk::builtin_events::PowerEvent;
//! use rmk_macro::controller;
//!
//! #[controller(subscribe = [PowerEvent])]
//! pub struct BatteryLedController {
//!     // ...
//! }
//!
//! impl BatteryLedController {
//!     async fn on_power_event(&mut self, event: PowerEvent) {
//!         match event {
//!             PowerEvent::Battery(level) => { /* handle battery */ },
//!             PowerEvent::Charging(charging) => { /* handle charging */ },
//!             PowerEvent::Sleep(sleep) => { /* handle sleep */ },
//!         }
//!     }
//! }
//! ```

use rmk_macro::controller_event;
use rmk_types::action::KeyAction;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;

#[cfg(feature = "_ble")]
use crate::ble::BleState;

// ============================================================================
// High-Frequency Events - PubSub Channel
// ============================================================================

/// Keyboard input events - key presses and modifier changes
///
/// This is a high-frequency event type that uses PubSubChannel with buffering.
/// Each event is important and should not be dropped.
#[controller_event(channel_size = 8, subs = 4)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyboardInputEvent {
    /// Key press/release with associated action
    Key {
        keyboard_event: KeyboardEvent,
        key_action: KeyAction,
    },
    /// Modifier keys combination changed
    Modifier(ModifierCombination),
}

impl KeyboardInputEvent {
    /// Create a key event
    pub fn key(keyboard_event: KeyboardEvent, key_action: KeyAction) -> Self {
        Self::Key {
            keyboard_event,
            key_action,
        }
    }

    /// Create a modifier event
    pub fn modifier(modifier: ModifierCombination) -> Self {
        Self::Modifier(modifier)
    }
}

// ============================================================================
// State Events - Watch Channel
// ============================================================================

/// Power management events - battery, charging, and sleep states
///
/// This is a state-based event using Watch channel.
/// Only the latest state matters, older states can be overridden.
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerEvent {
    /// Battery level changed (0-100)
    Battery(u8),
    /// Charging state changed (true = charging, false = not charging)
    Charging(bool),
    /// Sleep state changed (true = entering sleep, false = waking up)
    Sleep(bool),
}

impl PowerEvent {
    /// Create a battery level event
    pub fn battery(level: u8) -> Self {
        Self::Battery(level)
    }

    /// Create a charging state event
    pub fn charging(charging: bool) -> Self {
        Self::Charging(charging)
    }

    /// Create a sleep state event
    pub fn sleep(sleep: bool) -> Self {
        Self::Sleep(sleep)
    }
}

/// Keyboard state events - layer, WPM, and LED indicators
///
/// This is a state-based event using Watch channel.
/// Only the latest state matters, older states can be overridden.
#[controller_event(subs = 4)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyboardStateEvent {
    /// Active layer changed
    Layer(u8),
    /// Words per minute updated (typing speed)
    Wpm(u16),
    /// LED indicator state changed (NumLock, CapsLock, ScrollLock, etc.)
    Indicator(LedIndicator),
}

impl KeyboardStateEvent {
    /// Create a layer change event
    pub fn layer(layer: u8) -> Self {
        Self::Layer(layer)
    }

    /// Create a WPM update event
    pub fn wpm(wpm: u16) -> Self {
        Self::Wpm(wpm)
    }

    /// Create an indicator state event
    pub fn indicator(indicator: LedIndicator) -> Self {
        Self::Indicator(indicator)
    }
}

/// Connection type for USB/BLE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectionType {
    /// USB connection
    Usb,
    /// Bluetooth Low Energy connection
    Ble,
}

impl From<u8> for ConnectionType {
    fn from(value: u8) -> Self {
        match value {
            0 => ConnectionType::Usb,
            1 => ConnectionType::Ble,
            _ => ConnectionType::Usb, // default to USB
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

/// Connection events - USB/BLE connection state and BLE profile management
///
/// This is a state-based event using Watch channel.
/// Only the latest state matters, older states can be overridden.
#[cfg(feature = "_ble")]
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectionEvent {
    /// Connection type changed (USB or BLE)
    Type(ConnectionType),
    /// BLE profile state changed
    BleState {
        profile: u8,
        state: BleState,
    },
    /// Active BLE profile changed
    BleProfile(u8),
}

#[cfg(feature = "_ble")]
impl ConnectionEvent {
    /// Create a connection type event
    pub fn connection_type(conn_type: ConnectionType) -> Self {
        Self::Type(conn_type)
    }

    /// Create a BLE state event
    pub fn ble_state(profile: u8, state: BleState) -> Self {
        Self::BleState { profile, state }
    }

    /// Create a BLE profile change event
    pub fn ble_profile(profile: u8) -> Self {
        Self::BleProfile(profile)
    }
}

/// Split keyboard events - peripheral and central connection management
///
/// This is a state-based event using Watch channel.
/// Only the latest state matters, older states can be overridden.
#[cfg(feature = "split")]
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SplitEvent {
    /// Split peripheral connection state changed
    PeripheralConnected {
        id: usize,
        connected: bool,
    },
    /// Split peripheral battery level changed
    PeripheralBattery {
        id: usize,
        level: u8,
    },
    /// Split central connection state changed (from peripheral's perspective)
    CentralConnected(bool),
    /// Clear BLE peer information signal
    #[cfg(feature = "_ble")]
    ClearPeer,
}

#[cfg(feature = "split")]
impl SplitEvent {
    /// Create a peripheral connection event
    pub fn peripheral_connected(id: usize, connected: bool) -> Self {
        Self::PeripheralConnected { id, connected }
    }

    /// Create a peripheral battery event
    pub fn peripheral_battery(id: usize, level: u8) -> Self {
        Self::PeripheralBattery { id, level }
    }

    /// Create a central connection event
    pub fn central_connected(connected: bool) -> Self {
        Self::CentralConnected(connected)
    }

    /// Create a clear peer event
    #[cfg(feature = "_ble")]
    pub fn clear_peer() -> Self {
        Self::ClearPeer
    }
}
