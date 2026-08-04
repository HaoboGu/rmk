#[cfg(not(feature = "_ble"))]
use embedded_io_async::{Read, Write};

#[cfg(feature = "dfu_split")]
pub use crate::split::driver::UpdatePolicy;

/// Run the manager task of one serial split peripheral.
///
/// BLE split peripherals are managed inside the BLE transport — see
/// `BleTransport::run_split_central`.
#[cfg(not(feature = "_ble"))]
pub async fn run_peripheral_manager<S: Read + Write>(
    id: usize,
    receiver: S,
    matrix_config: crate::split::PeripheralMatrixConfig,
    #[cfg(feature = "dfu_split")] policy: crate::split::driver::UpdatePolicy,
) {
    crate::split::serial::run_serial_peripheral_manager(
        id,
        receiver,
        matrix_config,
        #[cfg(feature = "dfu_split")]
        policy,
    )
    .await;
}
