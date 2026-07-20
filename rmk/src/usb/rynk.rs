//! Rynk over USB CDC-ACM (Web Serial-compatible).

use embassy_usb::Builder;
use embassy_usb::class::cdc_acm::{BufferedReceiver, CdcAcmClass, Sender, State};
use embassy_usb::driver::Driver;
use embedded_io_async::{ErrorType, Write};
use static_cell::StaticCell;

use crate::host::rynk::RynkService;

#[cfg(feature = "_usb_high_speed")]
const RYNK_USB_MAX_PACKET_SIZE: u16 = 512;
#[cfg(not(feature = "_usb_high_speed"))]
const RYNK_USB_MAX_PACKET_SIZE: u16 = 64;

/// `BufferedReceiver` needs one packet worth of scratch to satisfy
/// sub-packet `Read::read` requests.
const RX_BUFFER_SIZE: usize = RYNK_USB_MAX_PACKET_SIZE as usize;

/// Reader/writer halves of the Rynk USB transport (CDC-ACM).
pub(crate) type HostUsbReader<D> = BufferedReceiver<'static, D>;
pub(crate) type HostUsbWriter<D> = Sender<'static, D>;

/// Build the Rynk CDC-ACM interface.
pub fn build_host_usb<D: Driver<'static>>(builder: &mut Builder<'static, D>) -> (HostUsbReader<D>, HostUsbWriter<D>) {
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
    receiver: &mut HostUsbReader<D>,
    sender: &mut HostUsbWriter<D>,
    service: &RynkService<'_>,
) -> ! {
    loop {
        sender.wait_connection().await;
        let mut tx = RynkUsbTx { sender: &mut *sender };
        service.run_session(receiver, &mut tx).await;
    }
}

/// Rynk USB writer: sends one frame per `write`, then a zero-length packet when
/// the frame fills the last bulk-IN packet. A CDC IN transfer completes on the
/// host only at a packet shorter than the max packet size, so a frame whose
/// length is a multiple of it would otherwise hang the host read (hit at
/// Full-Speed's 64-byte packets; masked at High-Speed's 512). `run_session`
/// writes each frame with a single `write_all`, so `buf` is one whole frame.
struct RynkUsbTx<'a, D: Driver<'static>> {
    sender: &'a mut Sender<'static, D>,
}

impl<D: Driver<'static>> ErrorType for RynkUsbTx<'_, D> {
    type Error = <Sender<'static, D> as ErrorType>::Error;
}

impl<D: Driver<'static>> Write for RynkUsbTx<'_, D> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.sender.write_all(buf).await?;
        let max_packet = self.sender.max_packet_size() as usize;
        if !buf.is_empty() && buf.len().is_multiple_of(max_packet) {
            self.sender.write(&[]).await?;
        }
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.sender.flush().await
    }
}
