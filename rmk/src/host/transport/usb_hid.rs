//! USB HID transport for Vial (fixed 32-byte reports).
//!
//! Owns an `embassy_usb::class::hid::HidReaderWriter<32, 32>` allocated on
//! the embassy-usb builder at setup time. A single `UsbHidRxTx` implements
//! both `HostRx` and `HostTx` because Vial's loop uses them sequentially
//! (read → process → write).

use embassy_usb::class::hid::{HidReaderWriter, ReadError};
use embassy_usb::driver::{Driver, EndpointError};

use crate::host::{HostError, HostRx, HostTx};

const USB_HID_FRAME: usize = 32;

fn map_read_error(e: ReadError) -> HostError {
    match e {
        ReadError::Disabled => HostError::Disconnected,
        ReadError::BufferOverflow => HostError::TransportOverflow,
        ReadError::Sync(_) => HostError::Io,
    }
}

fn map_endpoint_error(e: EndpointError) -> HostError {
    match e {
        EndpointError::Disabled => HostError::Disconnected,
        EndpointError::BufferOverflow => HostError::TransportOverflow,
    }
}

pub(crate) struct UsbHidRxTx<'d, D: Driver<'d>> {
    inner: HidReaderWriter<'d, D, USB_HID_FRAME, USB_HID_FRAME>,
}

impl<'d, D: Driver<'d>> UsbHidRxTx<'d, D> {
    pub(crate) fn new(inner: HidReaderWriter<'d, D, USB_HID_FRAME, USB_HID_FRAME>) -> Self {
        Self { inner }
    }
}

impl<'d, D: Driver<'d>> HostRx for UsbHidRxTx<'d, D> {
    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, HostError> {
        if buf.len() < USB_HID_FRAME {
            return Err(HostError::BufferTooSmall);
        }
        self.inner.read(&mut buf[..USB_HID_FRAME]).await.map_err(map_read_error)
    }
}

impl<'d, D: Driver<'d>> HostTx for UsbHidRxTx<'d, D> {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), HostError> {
        if bytes.len() > USB_HID_FRAME {
            return Err(HostError::FrameTooLarge);
        }
        let mut buf = [0u8; USB_HID_FRAME];
        buf[..bytes.len()].copy_from_slice(bytes);
        self.inner.write(&buf).await.map_err(map_endpoint_error)
    }
}
