use embassy_futures::select::{Either, select};
use embassy_usb::class::hid::{HidReaderWriter, ReadError};
use embassy_usb::driver::{Driver, EndpointError};
use rmk_types::connection::ConnectionType;

use crate::channel::{HOST_USB_REPLY, enqueue_host_request};

/// Drives the USB HID Vial endpoint: forwards 32-byte OUT reports into
/// `HOST_REQUEST_CHANNEL` and writes replies pulled from `HOST_USB_REPLY` back to
/// the IN endpoint. `select` keeps both directions on a single borrow because
/// `HidReaderWriter::split` consumes by value. The outer loop re-awaits
/// `rw.ready()` and clears stale replies on every (re)connect, so disconnect →
/// reconnect cycles fall back into a clean read/reply session.
pub(crate) async fn run_usb_host<'d, D: Driver<'d>>(rw: &mut HidReaderWriter<'d, D, 32, 32>) -> ! {
    let mut buf = [0u8; 32];
    loop {
        // `read`/`write` return `Disabled` until the host (re)configures the
        // interface; wait for both endpoints before entering the inner loop.
        rw.ready().await;
        // Drop any reply queued by `HostService` from a prior, now-stale session.
        HOST_USB_REPLY.clear();
        error!("Start wait");
        loop {
            match select(rw.read(&mut buf), HOST_USB_REPLY.receive()).await {
                Either::First(Ok(_)) => enqueue_host_request(ConnectionType::Usb, buf).await,
                Either::First(Err(ReadError::Disabled)) => break,
                Either::First(Err(e)) => error!("USB host read error: {:?}", e),
                Either::Second(reply) => match rw.write(&reply).await {
                    Ok(()) => {}
                    Err(EndpointError::Disabled) => break,
                    Err(e) => error!("USB host write error: {:?}", e),
                },
            }
        }
    }
}
