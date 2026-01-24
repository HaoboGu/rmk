//! Exposed channels which can be used to share data across devices & processors

use embassy_sync::channel::Channel;
pub use embassy_sync::{blocking_mutex, channel, pubsub, zerocopy_channel};
#[cfg(feature = "_ble")]
use {crate::ble::profile::BleProfileAction, embassy_sync::signal::Signal, rmk_types::led_indicator::LedIndicator};

use crate::event::{Event, KeyboardEvent};
use crate::hid::Report;
use crate::{EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE, RawMutex};
#[cfg(feature = "storage")]
use crate::{FLASH_CHANNEL_SIZE, storage::FlashOperationMessage};

/// Signal for control led indicator, it's used only in BLE keyboards, since BLE receiving is not async
#[cfg(feature = "_ble")]
pub static LED_SIGNAL: Signal<RawMutex, LedIndicator> = Signal::new();
/// Channel for key events only
pub static KEY_EVENT_CHANNEL: Channel<RawMutex, KeyboardEvent, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for all other events
pub static EVENT_CHANNEL: Channel<RawMutex, Event, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for keyboard report from input processors to hid writer/reader
pub static KEYBOARD_REPORT_CHANNEL: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE> = Channel::new();

// Sync messages from server to flash
#[cfg(feature = "storage")]
pub(crate) static FLASH_CHANNEL: Channel<RawMutex, FlashOperationMessage, FLASH_CHANNEL_SIZE> = Channel::new();
#[cfg(feature = "_ble")]
pub(crate) static BLE_PROFILE_CHANNEL: Channel<RawMutex, BleProfileAction, 1> = Channel::new();
