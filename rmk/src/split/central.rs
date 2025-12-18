#[cfg(feature = "_ble")]
use core::cell::RefCell;

#[cfg(not(feature = "_ble"))]
use embedded_io_async::{Read, Write};
#[cfg(feature = "_ble")]
use {
    bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy, LeSetScanParams},
    bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync},
    heapless::VecView,
    trouble_host::prelude::*,
};

/// Run central's peripheral manager task.
///
/// # Arguments
/// * `id` - peripheral id
/// * `addr` - (optional) peripheral's BLE static address. This argument is enabled only for nRF BLE split now
/// * `receiver` - (optional) serial port. This argument is enabled only for serial split now
pub async fn run_peripheral_manager<
    'a,
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    #[cfg(feature = "_ble")] C: Controller
        + ControllerCmdSync<LeSetScanParams>
        + ControllerCmdAsync<LeSetPhy>
        + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    #[cfg(not(feature = "_ble"))] S: Read + Write,
>(
    id: usize,
    #[cfg(feature = "_ble")] addr: &RefCell<VecView<Option<[u8; 6]>>>,
    #[cfg(feature = "_ble")] stack: &'a Stack<'a, C, DefaultPacketPool>,
    #[cfg(not(feature = "_ble"))] receiver: S,
) {
    #[cfg(feature = "_ble")]
    {
        use crate::split::ble::central::run_ble_peripheral_manager;
        run_ble_peripheral_manager::<C, ROW, COL, ROW_OFFSET, COL_OFFSET>(id, addr, stack).await;
    };

    #[cfg(not(feature = "_ble"))]
    {
        use crate::split::serial::run_serial_peripheral_manager;
        run_serial_peripheral_manager::<ROW, COL, ROW_OFFSET, COL_OFFSET, S>(id, receiver).await;
    };
}
