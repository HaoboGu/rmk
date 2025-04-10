//! Exposed channels which can be used to share data across devices & processors
//!
use crate::RawMutex;
pub use embassy_sync::blocking_mutex;
pub use embassy_sync::channel;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
pub use embassy_sync::zerocopy_channel;

#[cfg(feature = "_nrf_ble")]
use crate::ble::nrf::profile::BleProfileAction;
use crate::event::{Event, KeyEvent};
use crate::hid::Report;
#[cfg(feature = "_ble")]
use crate::light::LedIndicator;
#[cfg(feature = "storage")]
use crate::storage::FlashOperationMessage;
pub const EVENT_CHANNEL_SIZE: usize = 16;
pub const REPORT_CHANNEL_SIZE: usize = 16;

/// Signal for control led indicator, it's used only in BLE keyboards, since BLE receiving is not async
#[cfg(feature = "_ble")]
pub static LED_SIGNAL: Signal<RawMutex, LedIndicator> = Signal::new();
/// Channel for battery level updates
pub static BATTERY_LEVEL_SIGNAL: Signal<RawMutex, u8> = Signal::new();
/// Channel for key events only
pub static KEY_EVENT_CHANNEL: Channel<RawMutex, KeyEvent, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for all other events
pub static EVENT_CHANNEL: Channel<RawMutex, Event, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for keyboard report from input processors to hid writer/reader
pub static KEYBOARD_REPORT_CHANNEL: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE> = Channel::new();
/// Channel for reading vial reports from the host
pub(crate) static VIAL_READ_CHANNEL: Channel<RawMutex, [u8; 32], 4> = Channel::new();
// Sync messages from server to flash
#[cfg(feature = "storage")]
pub(crate) static FLASH_CHANNEL: Channel<RawMutex, FlashOperationMessage, 4> = Channel::new();
#[cfg(feature = "_nrf_ble")]
pub(crate) static BLE_PROFILE_CHANNEL: Channel<RawMutex, BleProfileAction, 1> = Channel::new();
