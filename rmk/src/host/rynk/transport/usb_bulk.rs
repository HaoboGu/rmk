//! USB bulk endpoint transport for rynk.
//!
//! Impls `postcard_rpc::server::{WireRx, WireTx}` directly:
//! - `UsbBulkRx` reads from an embassy-usb `EndpointOut`, accumulates bytes
//!   across reads, returns one COBS-decoded frame per `receive` call.
//! - `UsbBulkTx` holds a `Mutex<EndpointIn>` (because `WireTx::send_raw` is
//!   `&self` but `EndpointIn::write` is `&mut self`), encodes outgoing frames
//!   with `postcard::ser_flavors::Cobs`, writes them in MTU-sized chunks.
//!
//! The scratch buffer for partial-frame accumulation lives in `UsbBulkRx`.
//! The TX path uses a stack-local scratch per call — cheaper than threading
//! a buffer field through the mutex.
//!
//! Skeleton — concrete endpoint wiring lands when `new_usb_builder` reserves
//! bulk IN/OUT endpoints.

use core::fmt::Arguments;

use embassy_sync::mutex::Mutex;
use postcard::ser_flavors::{Cobs, Flavor, Slice};
use postcard::Serializer;
use postcard_rpc::header::{VarHeader, VarKeyKind};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use serde::Serialize;

use crate::RawMutex;

/// Largest COBS-encoded frame (incoming or outgoing). Sized for the largest
/// postcard-rpc header + request/response payload + COBS overhead.
const FRAME_MAX: usize = 256;
/// postcard-rpc header max size: 1-byte kind + 8-byte key + 4-byte seq.
const HEADER_MAX: usize = 13;

// ---------------------------------------------------------------------------
// RX
// ---------------------------------------------------------------------------

pub(crate) struct UsbBulkRx {
    // TODO: replace with `embassy_usb::driver::EndpointOut` once the bulk
    // interface is reserved on the embassy-usb Builder.
    scratch: [u8; FRAME_MAX],
    filled: usize,
}

impl UsbBulkRx {
    pub(crate) fn new() -> Self {
        Self {
            scratch: [0; FRAME_MAX],
            filled: 0,
        }
    }
}

impl WireRx for UsbBulkRx {
    type Error = WireRxErrorKind;

    async fn receive<'a>(&mut self, _buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        // When wired: loop {
        //     if let Some(zero) = self.scratch[..self.filled].iter().position(|&b| b == 0) {
        //         let frame_end = zero + 1;
        //         let decode = cobs::decode(&self.scratch[..frame_end], _buf);
        //         self.scratch.copy_within(frame_end..self.filled, 0);
        //         self.filled -= frame_end;
        //         return match decode { ... };
        //     }
        //     match endpoint_out.read(&mut self.scratch[self.filled..]).await { ... }
        // }
        todo!("wire embassy-usb bulk OUT endpoint")
    }
}

// ---------------------------------------------------------------------------
// TX
// ---------------------------------------------------------------------------

struct UsbBulkTxInner {
    // TODO: replace with `embassy_usb::driver::EndpointIn`.
    _priv: (),
}

pub(crate) struct UsbBulkTx {
    inner: Mutex<RawMutex, UsbBulkTxInner>,
}

impl UsbBulkTx {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(UsbBulkTxInner { _priv: () }),
        }
    }
}

impl WireTx for UsbBulkTx {
    type Error = WireTxErrorKind;

    async fn send<T: Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        let mut scratch = [0u8; FRAME_MAX];
        let mut flavor = Cobs::try_new(Slice::new(&mut scratch)).map_err(|_| WireTxErrorKind::Other)?;

        let mut hdr_buf = [0u8; HEADER_MAX];
        let (hdr_bytes, _) = hdr
            .write_to_slice(&mut hdr_buf)
            .ok_or(WireTxErrorKind::Other)?;
        flavor.try_extend(hdr_bytes).map_err(|_| WireTxErrorKind::Other)?;

        let mut ser = Serializer { output: flavor };
        msg.serialize(&mut ser).map_err(|_| WireTxErrorKind::Other)?;
        let encoded = ser.output.finalize().map_err(|_| WireTxErrorKind::Other)?;

        self.write_all(encoded).await
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut scratch = [0u8; FRAME_MAX];
        let mut flavor = Cobs::try_new(Slice::new(&mut scratch)).map_err(|_| WireTxErrorKind::Other)?;
        flavor.try_extend(buf).map_err(|_| WireTxErrorKind::Other)?;
        let encoded = flavor.finalize().map_err(|_| WireTxErrorKind::Other)?;

        self.write_all(encoded).await
    }

    async fn send_log_str(&self, _kkind: VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        // rynk logs via defmt/log directly; LoggingTopic path is unused.
        Ok(())
    }

    async fn send_log_fmt<'a>(&self, _kkind: VarKeyKind, _a: Arguments<'a>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl UsbBulkTx {
    async fn write_all(&self, _bytes: &[u8]) -> Result<(), WireTxErrorKind> {
        let _guard = self.inner.lock().await;
        // TODO: fragment `_bytes` into MTU-sized chunks and call
        // `_guard.endpoint_in.write(chunk).await` until drained.
        // Map endpoint errors to WireTxErrorKind::ConnectionClosed / Other.
        todo!("wire embassy-usb bulk IN endpoint")
    }
}
