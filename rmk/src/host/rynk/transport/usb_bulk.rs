//! USB bulk endpoint transport for rynk (COBS-framed postcard bytes).
//!
//! Skeleton only — the concrete impl lands when the bulk endpoint pair is
//! wired into `new_usb_builder`. Intended shape:
//! - `UsbBulkRx` owns an `embassy_usb::driver::EndpointOut`; `receive` reads
//!   bytes into a staging buffer up to the next COBS sentinel (0x00), then
//!   decodes in-place into `buf` and returns the trimmed slice.
//! - `UsbBulkTx` owns an `embassy_usb::driver::EndpointIn` inside a
//!   `Mutex` (required because `WireTx::send_raw` is `&self`, but
//!   `EndpointIn::write` is `&mut self`). `send_raw` COBS-encodes then
//!   writes in MTU-sized chunks. Topic publishers and endpoint responses
//!   share one `&UsbBulkTx` — the mutex serialises them.
//!
//! Framing mirrors `rmk/src/split/serial/` (COBS over embedded-io-async).

use core::fmt::Arguments;

use postcard_rpc::header::{VarHeader, VarKeyKind};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use serde::Serialize;

pub(crate) struct UsbBulkRx {
    _priv: (),
}

impl UsbBulkRx {
    pub(crate) fn new() -> Self {
        Self { _priv: () }
    }
}

impl WireRx for UsbBulkRx {
    type Error = WireRxErrorKind;

    async fn receive<'a>(&mut self, _buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
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

impl WireTx for UsbBulkTx {
    type Error = WireTxErrorKind;

    async fn send<T: Serialize + ?Sized>(&self, _hdr: VarHeader, _msg: &T) -> Result<(), Self::Error> {
        todo!("serialize header + payload, then send_raw")
    }

    async fn send_raw(&self, _buf: &[u8]) -> Result<(), Self::Error> {
        todo!("lock inner sender, COBS-encode, write in MTU chunks")
    }

    async fn send_log_str(&self, _kkind: VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        todo!("encode a LoggingTopic frame via send_raw")
    }

    async fn send_log_fmt<'a>(&self, _kkind: VarKeyKind, _a: Arguments<'a>) -> Result<(), Self::Error> {
        todo!("format into a scratch buffer, then send_log_str")
    }
}
