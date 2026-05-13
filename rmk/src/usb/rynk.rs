//! Rynk over USB CDC-ACM (Web Serial-compatible).
//!
//! Two free functions:
//! - [`build_rynk_cdc`] — register a CDC-ACM class on the `embassy_usb`
//!   builder, return the split `(Sender, BufferedReceiver)` halves.
//!   Both halves already implement [`embedded_io_async::Write`] /
//!   [`embedded_io_async::Read`] natively, so no adapter types are needed.
//! - [`run_rynk_cdc`] — the reconnect loop: await DTR, run one session,
//!   repeat. Returns `!`; suitable for direct use as a task body.

use embassy_usb::Builder;
use embassy_usb::class::cdc_acm::{BufferedReceiver, CdcAcmClass, Sender, State};
use embassy_usb::driver::Driver;
use static_cell::StaticCell;

use crate::host::rynk::RynkService;

/// Max packet size for the CDC-ACM bulk endpoints. Matches the `usb_log`
/// logger so all CDC-ACM interfaces in the composite device agree.
const RYNK_USB_MAX_PACKET_SIZE: u16 = 64;

/// `BufferedReceiver` needs one packet worth of scratch to satisfy
/// sub-packet `Read::read` requests.
const RX_BUFFER_SIZE: usize = RYNK_USB_MAX_PACKET_SIZE as usize;

/// Register a CDC-ACM function on `builder` and return its split halves.
/// Allocates `State` and the receiver scratch in `static` cells (only safe
/// to call once per program — same constraint as embassy-usb's other
/// `*::new` factories).
pub fn build_rynk_cdc<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
) -> (Sender<'static, D>, BufferedReceiver<'static, D>) {
    static STATE: StaticCell<State> = StaticCell::new();
    static RX_BUF: StaticCell<[u8; RX_BUFFER_SIZE]> = StaticCell::new();

    let state = STATE.init(State::new());
    let class = CdcAcmClass::new(builder, state, RYNK_USB_MAX_PACKET_SIZE);
    let (sender, receiver) = class.split();
    let receiver = receiver.into_buffered(RX_BUF.init([0; RX_BUFFER_SIZE]));
    (sender, receiver)
}

/// Reconnect loop. Awaits DTR from the host, runs one rynk session until
/// it returns (read/write error, host close), then loops back to wait for
/// the next DTR assertion.
pub async fn run_rynk_cdc<D: Driver<'static>>(
    sender: &mut Sender<'static, D>,
    receiver: &mut BufferedReceiver<'static, D>,
    service: &RynkService<'_>,
) -> ! {
    loop {
        sender.wait_connection().await;
        service.run_session(receiver, sender).await;
    }
}
