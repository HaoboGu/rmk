//! Rynk over BLE GATT.
//!
//! One public free function — [`run_rynk_ble`] — that owns the whole
//! per-connection lifecycle: clear the inbound chunk channel, construct
//! Rx/Tx adapters around the GATT plumbing, and call
//! [`RynkService::run_session`] once. Returns when the underlying
//! `embedded_io_async` halves error out (typically a disconnect); the
//! parent BLE task is the outer reconnect loop.

use embedded_io_async::{ErrorType, Read, Write};
use heapless::Vec;
use trouble_host::prelude::*;

use crate::ble::ble_server::Server;
use crate::channel::RYNK_BLE_RX_CHANNEL;
use crate::host::rynk::{RYNK_BLE_CHUNK_SIZE, RynkService};

/// Variable-length value type of the Rynk `input_data` / `output_data`
/// characteristics.
type RynkChunk = Vec<u8, RYNK_BLE_CHUNK_SIZE>;

/// Run one rynk session over `conn`. Clears any leftover RX chunks from a
/// prior connection, constructs Rx/Tx adapters in-place, and returns when
/// the session ends.
pub async fn run_rynk_ble<'stack, 'server, P: PacketPool>(
    server: &'server Server<'_>,
    conn: &GattConnection<'stack, 'server, P>,
    service: &RynkService<'_>,
) {
    RYNK_BLE_RX_CHANNEL.clear();
    let mut residual = RynkChunk::new();
    let mut head = 0usize;
    let mut rx = RynkBleRx {
        residual: &mut residual,
        head: &mut head,
    };
    let mut tx = RynkBleTx {
        input_data: server.rynk_service.input_data.clone(),
        conn,
    };
    service.run_session(&mut rx, &mut tx).await;
}

/// Error type for the BLE rynk transport. Always reports
/// `ConnectionReset` — the framing layer only cares about live vs. gone.
#[derive(Debug)]
struct RynkBleError;

impl core::fmt::Display for RynkBleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("BLE rynk transport closed")
    }
}

impl core::error::Error for RynkBleError {}

impl embedded_io_async::Error for RynkBleError {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        embedded_io_async::ErrorKind::ConnectionReset
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for RynkBleError {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "RynkBleError")
    }
}

/// Read half. Drains [`RYNK_BLE_RX_CHANNEL`] one chunk at a time, copying
/// as much as fits into the caller's buffer and stashing any leftover in
/// the supplied residual slot for the next `read` call.
struct RynkBleRx<'a> {
    residual: &'a mut RynkChunk,
    head: &'a mut usize,
}

impl ErrorType for RynkBleRx<'_> {
    type Error = RynkBleError;
}

impl Read for RynkBleRx<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        if *self.head == self.residual.len() {
            // Residual drained; pull the next chunk from the channel.
            let chunk = RYNK_BLE_RX_CHANNEL.receive().await;
            *self.residual = chunk;
            *self.head = 0;
        }
        let available = &self.residual[*self.head..];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        *self.head += n;
        Ok(n)
    }
}

/// Write half. Notifies the `input_data` characteristic in
/// `RYNK_BLE_CHUNK_SIZE`-byte slices.
struct RynkBleTx<'a, 'b, 'c, P: PacketPool> {
    input_data: Characteristic<RynkChunk>,
    conn: &'a GattConnection<'b, 'c, P>,
}

impl<P: PacketPool> ErrorType for RynkBleTx<'_, '_, '_, P> {
    type Error = RynkBleError;
}

impl<P: PacketPool> Write for RynkBleTx<'_, '_, '_, P> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        for chunk in buf.chunks(RYNK_BLE_CHUNK_SIZE) {
            // `chunks(RYNK_BLE_CHUNK_SIZE)` yields slices bounded by the
            // RynkChunk capacity, so from_slice cannot fail.
            let payload = RynkChunk::from_slice(chunk).expect("chunk size <= RYNK_BLE_CHUNK_SIZE");
            if let Err(e) = self.input_data.notify(self.conn, &payload).await {
                error!("Failed to notify Rynk reply: {:?}", e);
                return Err(RynkBleError);
            }
        }
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
