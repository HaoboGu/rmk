use trouble_host::prelude::*;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::channel::HOST_BLE_TX;
use crate::hid::ViaReport;

#[gatt_service(uuid = service::HUMAN_INTERFACE_DEVICE)]
pub(crate) struct VialService {
    #[characteristic(uuid = "2a4a", read, value = [0x01, 0x01, 0x00, 0x03])]
    pub(crate) hid_info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = ViaReport::desc().try_into().expect("Failed to convert ViaReport to [u8; 27]"))]
    pub(crate) report_map: [u8; 27],
    #[characteristic(uuid = "2a4c", write_without_response)]
    pub(crate) hid_control_point: u8,
    #[characteristic(uuid = "2a4e", read, write_without_response, value = 1)]
    pub(crate) protocol_mode: u8,
    #[descriptor(uuid = "2908", read, value = [0u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify)]
    pub(crate) input_data: [u8; 32],
    #[descriptor(uuid = "2908", read, value = [0u8, 2u8])]
    #[characteristic(uuid = "2a4d", read, write, write_without_response)]
    pub(crate) output_data: [u8; 32],
}

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
