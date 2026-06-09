//! Rynk over USB CDC-ACM (Web Serial-compatible).

use embassy_usb::Builder;
use embassy_usb::class::cdc_acm::{BufferedReceiver, CdcAcmClass, Sender, State};
use embassy_usb::driver::Driver;
use static_cell::StaticCell;

use crate::host::rynk::RynkService;

/// Max packet size for the CDC-ACM bulk endpoints.
const RYNK_USB_MAX_PACKET_SIZE: u16 = 64;

/// `BufferedReceiver` needs one packet worth of scratch to satisfy
/// sub-packet `Read::read` requests.
const RX_BUFFER_SIZE: usize = RYNK_USB_MAX_PACKET_SIZE as usize;

/// Build the Rynk CDC-ACM interface.
pub fn build_host_usb<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
) -> (BufferedReceiver<'static, D>, Sender<'static, D>) {
    static STATE: StaticCell<State> = StaticCell::new();
    static RX_BUF: StaticCell<[u8; RX_BUFFER_SIZE]> = StaticCell::new();

    let state = STATE.init(State::new());
    let class = CdcAcmClass::new(builder, state, RYNK_USB_MAX_PACKET_SIZE);
    let (sender, receiver) = class.split();
    let receiver = receiver.into_buffered(RX_BUF.init([0; RX_BUFFER_SIZE]));
    (receiver, sender)
}

/// Rynk session loop
pub async fn run_host_usb<D: Driver<'static>>(
    receiver: &mut BufferedReceiver<'static, D>,
    sender: &mut Sender<'static, D>,
    service: &RynkService<'_>,
) -> ! {
    loop {
        sender.wait_connection().await;
        service.run_session(receiver, sender).await;
    }
}
