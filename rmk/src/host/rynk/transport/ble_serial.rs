//! BLE serial transport for rynk (custom GATT service + COBS framing).
//!
//! Skeleton only — the custom GATT service with a write-without-response
//! "rx" characteristic and a notify "tx" characteristic is declared in
//! `crate::ble::host::rynk` as a placeholder; concrete wiring lands together
//! with `UsbBulkRx/Tx`.
//!
//! When implemented:
//! - Host → device writes land in a static channel fed by the GATT server.
//!   `BleSerialRx::receive` drains that channel one frame at a time, running
//!   COBS decode across characteristic write fragments.
//! - `BleSerialTx::send_raw` encodes `buf` with COBS and calls `notify` on
//!   the tx characteristic, fragmenting as needed to fit the connection MTU.
//!   Because `trouble_host::Characteristic::notify` takes `&self` (plus a
//!   `&GattConnection`), no interior mutex is required here — unlike
//!   `UsbBulkTx`.

use core::fmt::Arguments;

use postcard_rpc::header::{VarHeader, VarKeyKind};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use serde::Serialize;

pub(crate) struct BleSerialRx {
    _priv: (),
}

impl BleSerialRx {
    pub(crate) fn new() -> Self {
        Self { _priv: () }
    }
}

impl WireRx for BleSerialRx {
    type Error = WireRxErrorKind;

    async fn receive<'a>(&mut self, _buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        todo!("wire custom GATT rx characteristic + COBS decoder")
    }
}

pub(crate) struct BleSerialTx {
    _priv: (),
}

impl BleSerialTx {
    pub(crate) fn new() -> Self {
        Self { _priv: () }
    }
}

impl WireTx for BleSerialTx {
    type Error = WireTxErrorKind;

    async fn send<T: Serialize + ?Sized>(&self, _hdr: VarHeader, _msg: &T) -> Result<(), Self::Error> {
        todo!("serialize header + payload, then send_raw")
    }

    async fn send_raw(&self, _buf: &[u8]) -> Result<(), Self::Error> {
        todo!("COBS-encode, notify on tx characteristic, fragment to MTU")
    }

    async fn send_log_str(&self, _kkind: VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        todo!("encode a LoggingTopic frame via send_raw")
    }

    async fn send_log_fmt<'a>(&self, _kkind: VarKeyKind, _a: Arguments<'a>) -> Result<(), Self::Error> {
        todo!("format into a scratch buffer, then send_log_str")
    }
}
