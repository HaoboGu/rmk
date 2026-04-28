use trouble_host::prelude::*;

use crate::channel::HOST_BLE_TX;

/// Drains `HOST_BLE_TX` and forwards each reply to the Vial input characteristic via GATT
/// notify. The receive side (host -> device) lives in `gatt_events_task`, which pushes
/// directly into `HOST_REQUEST_CHANNEL` tagged `Ble`.
///
/// Cancellation-safe: dropping this future aborts any in-flight notify; the `try_receive`
/// drain on the next startup discards any stale reply queued by `HostService` after the
/// previous run was cancelled.
pub(crate) async fn run_ble_host<P: PacketPool>(
    input: Characteristic<[u8; 32]>,
    conn: &GattConnection<'_, '_, P>,
) -> ! {
    while HOST_BLE_TX.try_receive().is_ok() {}
    loop {
        let buf = HOST_BLE_TX.receive().await;
        debug!("Sending via report: {:?}", buf);
        if let Err(e) = input.notify(conn, &buf).await {
            error!("Failed to notify via report: {:?}", e);
        }
    }
}
