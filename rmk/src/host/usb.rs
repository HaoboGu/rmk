use embassy_futures::select::{Either, select};
use embassy_time::Timer;
use embassy_usb::class::hid::HidReaderWriter;
use embassy_usb::driver::Driver;
use rmk_types::connection::ConnectionType;

use crate::channel::{HOST_USB_REPLY, try_enqueue_host_request};

/// Drives the USB HID Vial endpoint: forwards 32-byte OUT reports into
/// `HOST_REQUEST_CHANNEL` and writes replies pulled from `HOST_USB_REPLY` back to
/// the IN endpoint. `select` keeps both directions on a single borrow because
/// `HidReaderWriter::split` consumes by value. The startup `clear()` discards
/// any reply queued by `HostService` after a previous cancelled run.
pub(crate) async fn run_usb_host<'d, D: Driver<'d>>(rw: &mut HidReaderWriter<'d, D, 32, 32>) -> ! {
    HOST_USB_REPLY.clear();
    let mut buf = [0u8; 32];
    loop {
        match select(rw.read(&mut buf), HOST_USB_REPLY.receive()).await {
            Either::First(Ok(_)) => try_enqueue_host_request(ConnectionType::Usb, buf),
            Either::First(Err(e)) => {
                error!("USB host read error: {:?}", e);
                Timer::after_millis(100).await;
            }
            Either::Second(reply) => {
                if let Err(e) = rw.write(&reply).await {
                    error!("USB host write error: {:?}", e);
                }
            }
        }
    }
}
