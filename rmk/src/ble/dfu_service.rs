use trouble_host::prelude::*;

const RESPONSE_BUF_SIZE: usize = 64;

/// Nordic Secure DFU GATT service (UUID `0xFE59`).
///
/// Exposes two characteristics:
/// - **DFU Control Point** — notifiable write channel for DFU protocol
///   commands (`Create`, `Select`, `CRC`, `Execute`, etc.).
/// - **DFU Packet** — `write_without_response` channel for raw firmware
///   chunk data.
#[gatt_service(uuid = "fe59")]
pub(crate) struct DfuService {
    #[characteristic(
        uuid = "8ec90001-f315-4f60-9fb8-838830daea50",
        write,
        notify,
        value = [0u8; RESPONSE_BUF_SIZE]
    )]
    pub(crate) dfu_control_point: [u8; RESPONSE_BUF_SIZE],

    #[characteristic(
        uuid = "8ec90002-f315-4f60-9fb8-838830daea50",
        write_without_response,
        value = [0u8; DFU_PACKET_BUF_SIZE]
    )]
    pub(crate) dfu_packet: [u8; DFU_PACKET_BUF_SIZE],
}

/// Buttonless DFU GATT service.
///
/// The DFU tool writes a specific value to the `buttonless_dfu` characteristic
/// to signal "entering DFU mode"; the device responds with `[0x20, 0x01]`.
/// The actual firmware transfer is only accepted once `dfu_lock` is unlocked
/// (or if `dfu_lock` is not enabled).
///
/// **This service is present for Nordic DFU protocol compatibility only.**
/// It performs no state change, no reset, and no firmware gating — the
/// handler merely echoes `[0x20, 0x01]` back via indication.  The DFU tool
/// (B.O.L.T.) does not even write to it.  Removing it would break Nordic
/// nRF Connect / nRF Toolbox recognition but would not affect B.O.L.T.
/// flashing.
#[gatt_service(uuid = "8ec90003-f315-4f60-9fb8-838830daea50")]
pub(crate) struct ButtonlessDfuService {
    #[characteristic(
        uuid = "8ec90004-f315-4f60-9fb8-838830daea50",
        write,
        indicate,
        value = [0u8; 2]
    )]
    pub(crate) buttonless_dfu: [u8; 2],
}

pub(crate) const DFU_PACKET_BUF_SIZE: usize = 247;
