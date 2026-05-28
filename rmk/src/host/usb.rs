//! Vial over USB HID (32-byte IN/OUT reports).
//!
//! Two free functions mirroring the rynk side:
//! - [`build_vial_hid`] — register the HID class on the builder, return
//!   the split `(HidReader<32>, HidWriter<32>)` pair.
//! - [`run_vial_hid`] — the reconnect loop: await endpoint ready, wrap
//!   the halves in 32-byte HID-report `Read`/`Write` adapters, run one
//!   session, repeat.

use embassy_usb::Builder;
use embassy_usb::class::hid::{HidReader, HidWriter};
use embassy_usb::driver::Driver;
use embedded_io_async::{ErrorType, Read, Write};

use crate::hid::ViaReport;
use crate::host::via::VialService;
use crate::usb::add_usb_reader_writer;

/// Build the Vial HID interface (32-byte input + 32-byte output reports).
pub fn build_vial_hid<D: Driver<'static>>(
    builder: &mut Builder<'static, D>,
) -> (HidReader<'static, D, 32>, HidWriter<'static, D, 32>) {
    let rw = add_usb_reader_writer!(builder, ViaReport, 32, 32, 32);
    rw.split()
}

/// Reconnect loop. Awaits host endpoint readiness, runs one Vial session,
/// then loops back to wait again. The Rx/Tx adapter pair is recreated each
/// iteration — they're zero-cost `&mut` borrows of `reader`/`writer`.
pub async fn run_vial_hid<D: Driver<'static>>(
    reader: &mut HidReader<'static, D, 32>,
    writer: &mut HidWriter<'static, D, 32>,
    service: &VialService<'_>,
) -> ! {
    loop {
        reader.ready().await;
        let mut rx = VialUsbRx { reader: &mut *reader };
        let mut tx = VialUsbTx { writer: &mut *writer };
        service.run_session(&mut rx, &mut tx).await;
    }
}

/// Error type for the Vial USB transport.
#[derive(Debug)]
struct VialUsbError;

impl core::fmt::Display for VialUsbError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Vial USB transport closed")
    }
}

impl core::error::Error for VialUsbError {}

impl embedded_io_async::Error for VialUsbError {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        embedded_io_async::ErrorKind::ConnectionReset
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for VialUsbError {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "VialUsbError")
    }
}

/// Read half. HID reports arrive as fixed 32-byte packets. Callers drive
/// this via `read_exact(&mut [u8; 32])`; smaller buffers would truncate a
/// packet and are rejected.
struct VialUsbRx<'a, D: Driver<'static>> {
    reader: &'a mut HidReader<'static, D, 32>,
}

impl<D: Driver<'static>> ErrorType for VialUsbRx<'_, D> {
    type Error = VialUsbError;
}

impl<D: Driver<'static>> Read for VialUsbRx<'_, D> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.len() < 32 {
            error!("VialUsbRx::read called with buf.len() = {} < 32", buf.len());
            return Err(VialUsbError);
        }
        match self.reader.read(&mut buf[..32]).await {
            Ok(n) => Ok(n),
            Err(e) => {
                error!("USB host read error: {:?}", e);
                Err(VialUsbError)
            }
        }
    }
}

/// Write half. Sends one 32-byte HID report per `write` call.
struct VialUsbTx<'a, D: Driver<'static>> {
    writer: &'a mut HidWriter<'static, D, 32>,
}

impl<D: Driver<'static>> ErrorType for VialUsbTx<'_, D> {
    type Error = VialUsbError;
}

impl<D: Driver<'static>> Write for VialUsbTx<'_, D> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let n = buf.len().min(32);
        match self.writer.write(&buf[..n]).await {
            Ok(()) => Ok(n),
            Err(e) => {
                error!("USB host write error: {:?}", e);
                Err(VialUsbError)
            }
        }
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
