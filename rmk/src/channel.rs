//! Exposed channels which can be used to share data across devices & processors

use embassy_sync::channel::Channel;
#[cfg(feature = "_ble")]
use embassy_sync::signal::Signal;
pub use embassy_sync::{blocking_mutex, channel, pubsub, zerocopy_channel};
#[cfg(feature = "_ble")]
use {crate::ble::profile::BleProfileAction, rmk_types::led_indicator::LedIndicator};

use crate::hid::Report;
#[cfg(feature = "storage")]
use crate::{FLASH_CHANNEL_SIZE, storage::FlashOperationMessage};
use crate::{REPORT_CHANNEL_SIZE, RawMutex};
#[cfg(feature = "host")]
use crate::VIAL_CHANNEL_SIZE;

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

/// Identifies which transport produced a Vial host request, so `HostService` can route
/// the reply back to the right per-transport TX channel.
#[cfg(feature = "host")]
#[derive(Copy, Clone, Debug)]
pub(crate) enum HostTransport {
    #[cfg(not(feature = "_no_usb"))]
    Usb,
    #[cfg(feature = "_ble")]
    Ble,
}

/// Vial host requests from any active transport (USB or BLE) to the central `HostService`.
/// Items carry the originating transport tag so replies can be routed back.
#[cfg(feature = "host")]
pub(crate) static HOST_REQUEST_CHANNEL: Channel<RawMutex, (HostTransport, [u8; 32]), VIAL_CHANNEL_SIZE> = Channel::new();

/// Per-transport reply slot for USB. Size 1: at most one in-flight reply per transport since
/// `HostService` is strictly serial. The transport's I/O loop drains this on startup to discard
/// any stale entry left over from a previously-cancelled run.
#[cfg(all(feature = "host", not(feature = "_no_usb")))]
pub(crate) static HOST_USB_TX: Channel<RawMutex, [u8; 32], 1> = Channel::new();

/// Per-transport reply slot for BLE. See `HOST_USB_TX` for the queueing/draining rationale.
#[cfg(all(feature = "host", feature = "_ble"))]
pub(crate) static HOST_BLE_TX: Channel<RawMutex, [u8; 32], 1> = Channel::new();
