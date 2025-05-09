//! Exposed channels which can be used to share data across devices & processors

use crate::event::{ControllerEvent, Event, KeyEvent};
use crate::hid::Report;
use crate::RawMutex;
#[cfg(feature = "storage")]
use crate::{storage::FlashOperationMessage, FLASH_CHANNEL_SIZE};
use crate::{
    CONTROLLER_CHANNEL_PUBS, CONTROLLER_CHANNEL_SIZE, CONTROLLER_CHANNEL_SUBS, EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE,
    VIAL_CHANNEL_SIZE,
};
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::{PubSubChannel, Publisher, Subscriber};
pub use embassy_sync::{blocking_mutex, channel, pubsub, zerocopy_channel};
#[cfg(feature = "_ble")]
use {crate::ble::trouble::profile::BleProfileAction, crate::light::LedIndicator, embassy_sync::signal::Signal};
#[cfg(feature = "split")]
use {
    crate::split::SplitMessage,
    crate::{SPLIT_MESSAGE_CHANNEL_SIZE, SPLIT_PERIPHERALS_NUM},
};

pub type ControllerSub<'a> = Subscriber<
    'a,
    RawMutex,
    ControllerEvent,
    CONTROLLER_CHANNEL_SIZE,
    CONTROLLER_CHANNEL_SUBS,
    CONTROLLER_CHANNEL_PUBS,
>;
pub type ControllerPub<'a> =
    Publisher<'a, RawMutex, ControllerEvent, CONTROLLER_CHANNEL_SIZE, CONTROLLER_CHANNEL_SUBS, CONTROLLER_CHANNEL_PUBS>;

/// Signal for control led indicator, it's used only in BLE keyboards, since BLE receiving is not async
#[cfg(feature = "_ble")]
pub static LED_SIGNAL: Signal<RawMutex, LedIndicator> = Signal::new();
/// Channel for key events only
pub static KEY_EVENT_CHANNEL: Channel<RawMutex, KeyEvent, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for all other events
pub static EVENT_CHANNEL: Channel<RawMutex, Event, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for keyboard report from input processors to hid writer/reader
pub static KEYBOARD_REPORT_CHANNEL: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE> = Channel::new();
/// Channel for controller events
pub static CONTROLLER_CHANNEL: PubSubChannel<
    RawMutex,
    ControllerEvent,
    CONTROLLER_CHANNEL_SIZE,
    CONTROLLER_CHANNEL_SUBS,
    CONTROLLER_CHANNEL_PUBS,
> = PubSubChannel::new();
/// Channel for reading vial reports from the host
pub(crate) static VIAL_READ_CHANNEL: Channel<RawMutex, [u8; 32], VIAL_CHANNEL_SIZE> = Channel::new();
// Sync messages from server to flash
#[cfg(feature = "storage")]
pub(crate) static FLASH_CHANNEL: Channel<RawMutex, FlashOperationMessage, FLASH_CHANNEL_SIZE> = Channel::new();
#[cfg(feature = "_ble")]
pub(crate) static BLE_PROFILE_CHANNEL: Channel<RawMutex, BleProfileAction, 1> = Channel::new();
// Channel for publish split messages to all peripherals
// TODO: replace 3 to the number of the peripherals
#[cfg(feature = "split")]
pub(crate) static SPLIT_MESSAGE_PUBLISHER: PubSubChannel<
    RawMutex,
    SplitMessage,
    SPLIT_MESSAGE_CHANNEL_SIZE,
    SPLIT_PERIPHERALS_NUM,
    1,
> = PubSubChannel::new();
