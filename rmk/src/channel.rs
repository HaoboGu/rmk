use crate::RawMutex;
use embassy_sync::channel::Channel;

#[cfg(feature = "_nrf_ble")]
use crate::ble::nrf::profile::BleProfileAction;
use crate::event::{Event, KeyEvent};
use crate::hid::Report;
use crate::light::LedIndicator;
use crate::storage::FlashOperationMessage;

pub const EVENT_CHANNEL_SIZE: usize = 16;
pub const REPORT_CHANNEL_SIZE: usize = 16;

/// Channel for control led indicator
pub static LED_CHANNEL: Channel<RawMutex, LedIndicator, 4> = Channel::new();
/// Channel for key events only
pub static KEY_EVENT_CHANNEL: Channel<RawMutex, KeyEvent, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for all events
pub static EVENT_CHANNEL: Channel<RawMutex, Event, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for keyboard report from input processors to hid writer/reader
pub(crate) static KEYBOARD_REPORT_CHANNEL: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE> =
    Channel::new();
/// Channel for reading vial reports from the host
pub(crate) static VIAL_READ_CHANNEL: Channel<RawMutex, [u8; 32], 4> = Channel::new();
// Sync messages from server to flash
pub(crate) static FLASH_CHANNEL: Channel<RawMutex, FlashOperationMessage, 4> = Channel::new();
#[cfg(feature = "_nrf_ble")]
pub(crate) static BLE_PROFILE_CHANNEL: Channel<RawMutex, BleProfileAction, 1> = Channel::new();
