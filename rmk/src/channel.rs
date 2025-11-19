//! Exposed channels which can be used to share data across devices & processors

use embassy_sync::channel::Channel;
#[cfg(any(feature = "split", feature = "controller"))]
use embassy_sync::pubsub::PubSubChannel;
pub use embassy_sync::{blocking_mutex, channel, pubsub, zerocopy_channel};
#[cfg(feature = "_ble")]
use {crate::ble::profile::BleProfileAction, embassy_sync::signal::Signal, rmk_types::led_indicator::LedIndicator};
#[cfg(feature = "controller")]
use {
    crate::event::ControllerEvent,
    crate::{CONTROLLER_CHANNEL_PUBS, CONTROLLER_CHANNEL_SIZE, CONTROLLER_CHANNEL_SUBS},
    embassy_sync::pubsub::{Publisher, Subscriber},
};

#[cfg(feature = "split")]
use crate::SPLIT_PERIPHERALS_NUM;
use crate::event::{Event, KeyboardEvent};
use crate::hid::Report;
use crate::{EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE, RawMutex};
#[cfg(feature = "storage")]
use crate::{FLASH_CHANNEL_SIZE, storage::FlashOperationMessage};

#[cfg(feature = "controller")]
const CONTROLLER_CHANNEL_FINAL_SIZE: usize = const {
    // Calculate CONTROLLER_CHANNEL_FINAL_SIZE at compile-time
    #[cfg(feature = "split")]
    {
        CONTROLLER_CHANNEL_SIZE + SPLIT_PERIPHERALS_NUM
    }
    #[cfg(not(feature = "split"))]
    {
        CONTROLLER_CHANNEL_SIZE
    }
};

#[cfg(feature = "controller")]
pub type ControllerSub = Subscriber<
    'static,
    RawMutex,
    ControllerEvent,
    CONTROLLER_CHANNEL_FINAL_SIZE,
    CONTROLLER_CHANNEL_SUBS,
    CONTROLLER_CHANNEL_PUBS,
>;
#[cfg(feature = "controller")]
pub type ControllerPub = Publisher<
    'static,
    RawMutex,
    ControllerEvent,
    CONTROLLER_CHANNEL_FINAL_SIZE,
    CONTROLLER_CHANNEL_SUBS,
    CONTROLLER_CHANNEL_PUBS,
>;

/// Signal for control led indicator, it's used only in BLE keyboards, since BLE receiving is not async
#[cfg(feature = "_ble")]
pub static LED_SIGNAL: Signal<RawMutex, LedIndicator> = Signal::new();
/// Channel for key events only
pub static KEY_EVENT_CHANNEL: Channel<RawMutex, KeyboardEvent, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for all other events
pub static EVENT_CHANNEL: Channel<RawMutex, Event, EVENT_CHANNEL_SIZE> = Channel::new();
/// Channel for keyboard report from input processors to hid writer/reader
pub static KEYBOARD_REPORT_CHANNEL: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE> = Channel::new();
/// Channel for controller events
#[cfg(feature = "controller")]
pub static CONTROLLER_CHANNEL: PubSubChannel<
    RawMutex,
    ControllerEvent,
    CONTROLLER_CHANNEL_FINAL_SIZE,
    CONTROLLER_CHANNEL_SUBS,
    CONTROLLER_CHANNEL_PUBS,
> = PubSubChannel::new();

// Sync messages from server to flash
#[cfg(feature = "storage")]
pub(crate) static FLASH_CHANNEL: Channel<RawMutex, FlashOperationMessage, FLASH_CHANNEL_SIZE> = Channel::new();
#[cfg(feature = "_ble")]
pub(crate) static BLE_PROFILE_CHANNEL: Channel<RawMutex, BleProfileAction, 1> = Channel::new();

/// Send the specified `event` to `CONTROLLER_CHANNEL`.
#[cfg(feature = "controller")]
pub fn send_controller_event(publisher: &mut ControllerPub, event: ControllerEvent) {
    info!("Sending ControllerEvent: {:?}", event);
    publisher.publish_immediate(event);
}

/// Send the specified `event` to `CONTROLLER_CHANNEL`.
/// Do not use this function if you plan to send events multiple times, use `send_controller_event`
/// instead for better performance.
#[cfg(feature = "controller")]
pub fn send_controller_event_new(event: ControllerEvent) {
    if let Ok(mut publisher) = CONTROLLER_CHANNEL.publisher() {
        send_controller_event(publisher, event);
    }
}
