//! Exposed channels which can be used to share data across devices & processors

use core::future::poll_fn;

use embassy_sync::channel::{Channel, TrySendError};
#[cfg(feature = "_ble")]
use embassy_sync::signal::Signal;
pub use embassy_sync::{blocking_mutex, channel, pubsub, zerocopy_channel};
use rmk_types::connection::ConnectionType;
#[cfg(feature = "_ble")]
use {crate::ble::profile::BleProfileAction, rmk_types::led_indicator::LedIndicator};

#[cfg(feature = "host")]
use crate::VIAL_CHANNEL_SIZE;
use crate::hid::Report;
#[cfg(feature = "storage")]
use crate::{FLASH_CHANNEL_SIZE, storage::FlashOperationMessage};
use crate::{REPORT_CHANNEL_SIZE, RawMutex};

type ReportChannel = Channel<RawMutex, Report, REPORT_CHANNEL_SIZE>;

/// Signal for LED indicator, used in BLE keyboards only since BLE receiving is not async
#[cfg(feature = "_ble")]
pub(crate) static LED_SIGNAL: Signal<RawMutex, LedIndicator> = Signal::new();

/// Drained by the USB HID writer task. Routed through `send_hid_report`
/// from the keyboard task and ad-hoc producers (e.g. steno chord output).
#[cfg(not(feature = "_no_usb"))]
pub static USB_REPORT_CHANNEL: ReportChannel = Channel::new();

/// Drained by the BLE HID writer task. Routed through `send_hid_report`.
#[cfg(feature = "_ble")]
pub static BLE_REPORT_CHANNEL: ReportChannel = Channel::new();

fn active_report_channel() -> Option<(ConnectionType, &'static ReportChannel)> {
    match crate::state::active_transport()? {
        #[cfg(not(feature = "_no_usb"))]
        ConnectionType::Usb => Some((ConnectionType::Usb, &USB_REPORT_CHANNEL)),
        #[cfg(feature = "_ble")]
        ConnectionType::Ble => Some((ConnectionType::Ble, &BLE_REPORT_CHANNEL)),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

/// Reports generated while no transport is selected are dropped on the floor.
pub(crate) async fn send_hid_report(mut report: Report) {
    let Some((transport, ch)) = active_report_channel() else {
        return;
    };

    loop {
        match ch.try_send(report) {
            Ok(()) => return,
            Err(TrySendError::Full(r)) => report = r,
        }

        poll_fn(|cx| ch.poll_ready_to_send(cx)).await;
        if crate::state::active_transport() != Some(transport) {
            return;
        }
    }
}

/// Drops the report when the active transport's queue is full or no
/// transport is selected. Use for producers where back-pressure would block
/// the matrix scan (e.g. steno chord output).
pub(crate) fn try_send_hid_report(report: Report) {
    if let Some((_, ch)) = active_report_channel() {
        let _ = ch.try_send(report);
    }
}

/// Drains queued reports for `transport`. Called on active-transport flips so
/// a future re-activation doesn't replay stale presses without their releases.
pub(crate) fn clear_report_channel(transport: ConnectionType) {
    match transport {
        #[cfg(not(feature = "_no_usb"))]
        ConnectionType::Usb => USB_REPORT_CHANNEL.clear(),
        #[cfg(feature = "_ble")]
        ConnectionType::Ble => BLE_REPORT_CHANNEL.clear(),
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

// Sync messages from server to flash
#[cfg(feature = "storage")]
pub(crate) static FLASH_CHANNEL: Channel<RawMutex, FlashOperationMessage, FLASH_CHANNEL_SIZE> = Channel::new();
#[cfg(feature = "_ble")]
pub(crate) static BLE_PROFILE_CHANNEL: Channel<RawMutex, BleProfileAction, 1> = Channel::new();

/// Vial host requests from any active transport (USB or BLE) to the central `HostService`.
/// Items carry the originating transport tag so replies can be routed back to the right
/// per-transport reply channel.
///
/// Note: `HostService` processes requests strictly serially, so a slow request from one
/// transport (e.g. flash-bound `process_vial`) blocks queries from the other transport
/// queued behind it until it completes.
#[cfg(feature = "host")]
pub(crate) static HOST_REQUEST_CHANNEL: Channel<RawMutex, (ConnectionType, [u8; 32]), VIAL_CHANNEL_SIZE> =
    Channel::new();

/// Per-transport reply for USB. Capacity matches the request queue so bursts of
/// host requests can keep their replies queued until the transport drains them.
#[cfg(all(feature = "host", not(feature = "_no_usb")))]
pub(crate) static HOST_USB_REPLY: Channel<RawMutex, [u8; 32], VIAL_CHANNEL_SIZE> = Channel::new();

/// Per-transport reply for BLE. See `HOST_USB_REPLY` for the sizing/draining rationale.
#[cfg(all(feature = "host", feature = "_ble"))]
pub(crate) static HOST_BLE_REPLY: Channel<RawMutex, [u8; 32], VIAL_CHANNEL_SIZE> = Channel::new();

/// Routes a Vial reply back to the channel owned by the originating transport.
/// Drops with a warning when the destination queue already has a pending reply
/// (the `HostService` produced faster than the transport drained it).
#[cfg(feature = "host")]
pub(crate) fn try_send_host_reply(transport: ConnectionType, reply: [u8; 32]) {
    let ok = match transport {
        #[cfg(not(feature = "_no_usb"))]
        ConnectionType::Usb => HOST_USB_REPLY.try_send(reply).is_ok(),
        #[cfg(feature = "_ble")]
        ConnectionType::Ble => HOST_BLE_REPLY.try_send(reply).is_ok(),
        #[allow(unreachable_patterns)]
        _ => false,
    };
    if !ok {
        warn!("Dropping Vial {:?} reply: reply queue full", transport);
    }
}

/// Enqueues a Vial request from a transport into `HOST_REQUEST_CHANNEL`. Logs
/// and drops when the queue is full.
#[cfg(feature = "host")]
pub(crate) fn try_enqueue_host_request(transport: ConnectionType, data: [u8; 32]) {
    if HOST_REQUEST_CHANNEL.try_send((transport, data)).is_err() {
        warn!(
            "Dropping Vial {:?} request because the request queue is full",
            transport
        );
    }
}
