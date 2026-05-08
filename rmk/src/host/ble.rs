use trouble_host::prelude::*;

#[cfg(feature = "vial")]
use crate::channel::HOST_BLE_REPLY;

/// Drains `HOST_BLE_REPLY` and forwards each reply to the Vial input characteristic
/// via GATT notify. The startup `clear()` discards any reply queued by
/// `HostService` after a previous cancelled run.
#[cfg(feature = "vial")]
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

/// Drains `RMK_PROTOCOL_REPLY_CHANNEL` and forwards each COBS-encoded frame to
/// the rmk_protocol input characteristic via GATT notify.
///
/// Wire framing: each frame is COBS-encoded with a `0x00` sentinel and may
/// span multiple BLE notifications (frame length > MTU − 3). The host's
/// reframer accumulates chunks until it sees the sentinel; we send each chunk
/// as a borrowed slice (`to_raw()`) at its true length, so a sub-MTU final
/// chunk does not get zero-padded into a spurious sentinel.
///
/// On connection drop we clear both the request and reply channels and reset
/// the ready signal so frames queued during the previous session are not
/// delivered to the new client.
#[cfg(feature = "rmk_protocol")]
pub(crate) async fn run_ble_rmk_protocol<P: PacketPool>(
    input: Characteristic<[u8; 244]>,
    conn: &GattConnection<'_, '_, P>,
) -> ! {
    use crate::channel::{BLE_RMK_PROTOCOL_READY, RMK_PROTOCOL_REPLY_CHANNEL, RMK_PROTOCOL_REQUEST_CHANNEL};
    use crate::host::rmk_protocol::wire_ble::BLE_NOTIFY_PAYLOAD;
    let raw = input.to_raw();
    RMK_PROTOCOL_REPLY_CHANNEL.clear();
    RMK_PROTOCOL_REQUEST_CHANNEL.clear();
    BLE_RMK_PROTOCOL_READY.reset();
    loop {
        let frame = RMK_PROTOCOL_REPLY_CHANNEL.receive().await;
        let mut offset = 0;
        while offset < frame.len() {
            let take = (frame.len() - offset).min(BLE_NOTIFY_PAYLOAD);
            let slice = &frame[offset..offset + take];
            if let Err(e) = raw.notify(conn, slice).await {
                error!("Failed to notify rmk_protocol frame: {:?}", e);
                break;
            }
            offset += take;
        }
    }
}
