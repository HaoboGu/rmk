//! Exposed channels which can be used to share data across devices & processors

use embassy_sync::channel::Channel;
#[cfg(any(feature = "_ble", feature = "host"))]
use embassy_sync::signal::Signal;
pub use embassy_sync::{blocking_mutex, channel, pubsub, zerocopy_channel};
#[cfg(feature = "_ble")]
use {crate::ble::profile::BleProfileAction, rmk_types::led_indicator::LedIndicator};

use crate::hid::Report;
#[cfg(feature = "storage")]
use crate::{FLASH_CHANNEL_SIZE, storage::FlashOperationMessage};
use crate::{REPORT_CHANNEL_SIZE, RawMutex};
#[cfg(feature = "host")]
use crate::{VIAL_CHANNEL_SIZE, hid::ViaReport};

/// Signal for LED indicator, used in BLE keyboards only since BLE receiving is not async
#[cfg(feature = "_ble")]
pub(crate) static LED_SIGNAL: Signal<RawMutex, LedIndicator> = Signal::new();
/// Channel for keyboard report from input processors to hid writer/reader
pub static KEYBOARD_REPORT_CHANNEL: Channel<RawMutex, Report, REPORT_CHANNEL_SIZE> = Channel::new();

// Sync messages from server to flash
#[cfg(feature = "storage")]
pub(crate) static FLASH_CHANNEL: Channel<RawMutex, FlashOperationMessage, FLASH_CHANNEL_SIZE> = Channel::new();
#[cfg(feature = "_ble")]
pub(crate) static BLE_PROFILE_CHANNEL: Channel<RawMutex, BleProfileAction, 1> = Channel::new();

/// Vial host requests: each entry is `(request, &reply_signal)`. The transport
/// bridge fills `request` from real hardware (USB HID or BLE GATT), then awaits
/// its reply_signal. The top-level `HostService` consumes the channel, processes
/// the request, and signals the reply slot. Per-bridge signals avoid cross-transport
/// response confusion when USB and BLE both have active host sessions.
#[cfg(feature = "host")]
pub(crate) static HOST_REQUEST_CHANNEL: Channel<
    RawMutex,
    (ViaReport, &'static Signal<RawMutex, ViaReport>),
    VIAL_CHANNEL_SIZE,
> = Channel::new();
