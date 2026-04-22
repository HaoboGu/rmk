use embassy_sync::channel::Channel;
use trouble_host::prelude::*;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::descriptor::ViaReport;
use crate::{RawMutex, VIAL_CHANNEL_SIZE};

#[gatt_service(uuid = service::HUMAN_INTERFACE_DEVICE)]
pub(crate) struct VialGattService {
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

/// Channel from the GATT write handler to the Vial BLE HID transport.
///
/// Fed by [`handle_write`] when the host writes a 32-byte frame to the Vial
/// output characteristic. Drained by `crate::host::via::transport::ble_hid::BleHidRxTx`.
pub(crate) static VIAL_OUTPUT_CHANNEL: Channel<RawMutex, [u8; 32], VIAL_CHANNEL_SIZE> = Channel::new();

/// GATT attribute handle of Vial's notifiable characteristic's CCCD.
///
/// Used by the BLE event loop to detect when the host toggles notifications.
pub(crate) fn host_cccd_handle(gatt: &VialGattService) -> u16 {
    gatt.input_data.cccd_handle.expect("No CCCD for Vial input_data")
}

/// Handle a GATT write targeted at the Vial service.
///
/// Returns `true` if the event was consumed (write matched a Vial
/// characteristic), `false` if it belongs to some other service.
pub(crate) async fn handle_write(gatt: &VialGattService, event_handle: u16, event_data: &[u8]) -> bool {
    if event_handle == gatt.output_data.handle {
        debug!("Got host packet: {:?}", event_data);
        if event_data.len() == 32 {
            let mut data = [0u8; 32];
            data.copy_from_slice(event_data);
            VIAL_OUTPUT_CHANNEL.send(data).await;
        } else {
            warn!("Wrong host packet data: {:?}", event_data);
        }
        return true;
    }

    if event_handle == gatt.hid_control_point.handle {
        info!("Write GATT Event to host Control Point: {:?}", event_handle);
        #[cfg(feature = "split")]
        if event_data.len() == 1 {
            let data = event_data[0];
            if data == 0 {
                crate::split::ble::central::CENTRAL_SLEEP.signal(true);
            } else if data == 1 {
                crate::split::ble::central::CENTRAL_SLEEP.signal(false);
            }
        }
        return true;
    }

    false
}
