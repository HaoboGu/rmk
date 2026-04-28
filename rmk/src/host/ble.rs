use trouble_host::prelude::*;

use crate::channel::HOST_BLE_REPLY;

/// Drains `HOST_BLE_REPLY` and forwards each reply to the Vial input characteristic
/// via GATT notify. The startup `clear()` discards any reply queued by
/// `HostService` after a previous cancelled run.
pub(crate) async fn run_ble_host<P: PacketPool>(
    input: Characteristic<[u8; 32]>,
    conn: &GattConnection<'_, '_, P>,
) -> ! {
    HOST_BLE_REPLY.clear();
    loop {
        let buf = HOST_BLE_REPLY.receive().await;
        debug!("Sending via report: {:?}", buf);
        if let Err(e) = input.notify(conn, &buf).await {
            error!("Failed to notify via report: {:?}", e);
        }
    }
}
