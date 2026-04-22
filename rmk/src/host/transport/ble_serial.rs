//! BLE serial transport for rynk (custom GATT service + COBS framing).
//!
//! Skeleton only — the custom GATT service with a write-without-response
//! "rx" characteristic and a notify "tx" characteristic will be defined
//! alongside `ble/battery_service.rs` and wired in `ble/mod.rs`.
//!
//! When implemented:
//! - Host → device writes land in a static channel fed by the GATT server.
//!   `BleSerialRx::recv` drains that channel one frame at a time, running
//!   COBS decode across characteristic write fragments.
//! - `BleSerialTx::send` encodes `bytes` with COBS and calls `notify` on
//!   the tx characteristic, fragmenting as needed to fit the connection MTU.

use crate::host::{HostError, HostRx, HostTx};

pub(crate) struct BleSerialRx {
    _priv: (),
}

impl BleSerialRx {
    pub(crate) fn new() -> Self {
        Self { _priv: () }
    }
}

impl HostRx for BleSerialRx {
    async fn recv(&mut self, _buf: &mut [u8]) -> Result<usize, HostError> {
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

impl HostTx for BleSerialTx {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), HostError> {
        todo!("wire custom GATT tx characteristic + COBS encoder")
    }
}
