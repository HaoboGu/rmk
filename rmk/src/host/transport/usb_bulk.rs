//! USB bulk endpoint transport for rynk (COBS-framed postcard bytes).
//!
//! Skeleton only — concrete impl lands once the bulk endpoint setup is
//! wired into `new_usb_builder`. The intended shape is:
//! - `UsbBulkRx` owns a `Receiver<'d, D>` from `embassy_usb::driver`; recv
//!   reads bytes until the next COBS sentinel (0x00) and decodes into `buf`.
//! - `UsbBulkTx` owns a `Sender<'d, D>`; send encodes `bytes` with COBS and
//!   writes the framed output in MTU-sized chunks.
//!
//! Framing mirrors `rmk/src/split/serial/` (COBS over embedded-io-async).

use crate::host::{HostError, HostRx, HostTx};

pub(crate) struct UsbBulkRx {
    _priv: (),
}

impl UsbBulkRx {
    pub(crate) fn new() -> Self {
        Self { _priv: () }
    }
}

impl HostRx for UsbBulkRx {
    async fn recv(&mut self, _buf: &mut [u8]) -> Result<usize, HostError> {
        todo!("wire embassy-usb bulk OUT endpoint + COBS decoder")
    }
}

pub(crate) struct UsbBulkTx {
    _priv: (),
}

impl UsbBulkTx {
    pub(crate) fn new() -> Self {
        Self { _priv: () }
    }
}

impl HostTx for UsbBulkTx {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), HostError> {
        todo!("wire embassy-usb bulk IN endpoint + COBS encoder")
    }
}
