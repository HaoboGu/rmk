use trouble_host::prelude::*;

use super::HostGatt;

/// Stub custom GATT service for the rynk protocol.
///
/// The real service will expose a write-without-response "rx" characteristic
/// (host→device, COBS-framed postcard bytes) and a notify "tx" characteristic
/// (device→host, same framing). This placeholder exists so the BLE server
/// struct compiles under `rmk_protocol + _ble`; concrete characteristic
/// wiring is a follow-up.
///
/// TODO(rynk-ble): the UUIDs below are placeholders derived from the ASCII
/// bytes of "RMK " (0x52 0x44 0x4d 0x4b). Allocate a proper UUID space and
/// match any host-side client library before shipping.
#[gatt_service(uuid = "52444d4b-0000-1000-8000-00805f9b34fb")]
pub(crate) struct RynkGattService {
    #[characteristic(uuid = "52444d4b-0001-1000-8000-00805f9b34fb", write_without_response, value = [0u8; 64])]
    pub(crate) rx: [u8; 64],
    #[characteristic(uuid = "52444d4b-0002-1000-8000-00805f9b34fb", read, notify, value = [0u8; 64])]
    pub(crate) tx: [u8; 64],
}

impl HostGatt for RynkGattService {
    fn host_cccd_handle(&self) -> u16 {
        todo!("wire rynk tx CCCD handle when rynk transport is implemented")
    }

    async fn handle_write(&self, _event_handle: u16, _event_data: &[u8]) -> bool {
        todo!("wire rynk rx characteristic writes into the rynk transport's frame channel")
    }
}
