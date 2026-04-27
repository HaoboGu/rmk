use embassy_futures::select::{Either, select};
use embassy_time::Timer;
use embassy_usb::class::hid::HidReaderWriter;
use embassy_usb::driver::Driver;

use crate::channel::{HOST_REQUEST_CHANNEL, HOST_USB_TX, HostTransport};

/// Drives the USB HID Vial endpoint: forwards 32-byte OUT reports into `HOST_REQUEST_CHANNEL`
/// (tagged `Usb`) and writes replies pulled from `HOST_USB_TX` back to the IN endpoint.
///
/// `select`-based so a single `&mut HidReaderWriter` borrow can serve both directions without
/// `split()` (which consumes by value). Cancellation-safe: dropping this future cleanly aborts
/// any in-flight read/write, and the `try_receive` drain on the next startup discards any reply
/// that landed in `HOST_USB_TX` after the previous run was cancelled.
pub(crate) async fn run_usb_host<'d, D: Driver<'d>>(rw: &mut HidReaderWriter<'d, D, 32, 32>) -> ! {
    while HOST_USB_TX.try_receive().is_ok() {}
    let mut buf = [0u8; 32];
    loop {
        match select(rw.read(&mut buf), HOST_USB_TX.receive()).await {
            Either::First(Ok(_)) => HOST_REQUEST_CHANNEL.send((HostTransport::Usb, buf)).await,
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
