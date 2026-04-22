//! BLE serial transport for rynk.
//!
//! Impls `postcard_rpc::server::{WireRx, WireTx}` directly against the rynk
//! custom GATT service (see `crate::ble::host::rynk`):
//! - `BleSerialRx` drains host→device characteristic writes from a static
//!   channel fed by `RynkGattService::handle_write`, reassembles
//!   COBS-framed frames that may span multiple GATT writes (if a frame
//!   exceeds the ATT MTU).
//! - `BleSerialTx` calls `Characteristic::notify` on the tx characteristic,
//!   fragmenting outgoing frames to fit MTU. No interior mutex — both
//!   `notify` and our `WireTx::send_raw` are `&self`, so the `&self` chain
//!   is unbroken.
//!
//! COBS is retained (rather than one-frame-per-GATT-write) because a single
//! postcard-rpc frame can exceed MTU; COBS gives us a clean sentinel-based
//! reassembly story identical to the USB bulk transport.
//!
//! Skeleton — concrete GATT wiring lands when `RynkGattService::handle_write`
//! is implemented.

use core::fmt::Arguments;

use postcard::ser_flavors::{Cobs, Flavor, Slice};
use postcard::Serializer;
use postcard_rpc::header::{VarHeader, VarKeyKind};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use serde::Serialize;

/// Largest COBS-encoded frame handled in either direction.
const FRAME_MAX: usize = 256;
/// postcard-rpc header max size: 1-byte kind + 8-byte key + 4-byte seq.
const HEADER_MAX: usize = 13;

// ---------------------------------------------------------------------------
// RX
// ---------------------------------------------------------------------------

pub(crate) struct BleSerialRx {
    // TODO: hold a `Receiver<'static, RawMutex, heapless::Vec<u8, N>, _>`
    // for the static channel `RynkGattService::handle_write` feeds.
    scratch: [u8; FRAME_MAX],
    filled: usize,
}

impl BleSerialRx {
    pub(crate) fn new() -> Self {
        Self {
            scratch: [0; FRAME_MAX],
            filled: 0,
        }
    }
}

impl WireRx for BleSerialRx {
    type Error = WireRxErrorKind;

    async fn receive<'a>(&mut self, _buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        // When wired: same shape as UsbBulkRx::receive, but the byte source is
        // `channel.receive().await` (one GATT write's worth of bytes) instead
        // of `endpoint_out.read`. COBS decode is identical.
        todo!("drain GATT rx channel into scratch; cobs::decode on sentinel")
    }
}

// ---------------------------------------------------------------------------
// TX
// ---------------------------------------------------------------------------

pub(crate) struct BleSerialTx {
    // TODO: hold `Characteristic<[u8; N]>` + `&'conn GattConnection<...>`.
    _priv: (),
}

impl BleSerialTx {
    pub(crate) fn new() -> Self {
        Self { _priv: () }
    }
}

impl WireTx for BleSerialTx {
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

        self.notify_fragmented(encoded).await
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut scratch = [0u8; FRAME_MAX];
        let mut flavor = Cobs::try_new(Slice::new(&mut scratch)).map_err(|_| WireTxErrorKind::Other)?;
        flavor.try_extend(buf).map_err(|_| WireTxErrorKind::Other)?;
        let encoded = flavor.finalize().map_err(|_| WireTxErrorKind::Other)?;

        self.notify_fragmented(encoded).await
    }

    async fn send_log_str(&self, _kkind: VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        // rynk logs via defmt/log directly; LoggingTopic path is unused.
        Ok(())
    }

    async fn send_log_fmt<'a>(&self, _kkind: VarKeyKind, _a: Arguments<'a>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl BleSerialTx {
    async fn notify_fragmented(&self, _bytes: &[u8]) -> Result<(), WireTxErrorKind> {
        // TODO: chunk `_bytes` into MTU-sized pieces and call
        // `self.characteristic.notify(self.conn, chunk).await` on each.
        // Map notify errors to WireTxErrorKind::ConnectionClosed / Other.
        todo!("fragment to MTU and notify on rynk tx characteristic")
    }
}
