pub(crate) mod context;
#[cfg(feature = "rynk")]
pub(crate) mod rynk;
#[cfg(feature = "storage")]
pub(crate) mod storage;
// Shared transport-adapter error, used by the USB/BLE Vial and BLE Rynk
// adapters. Gated to exactly the feature combos that compile an adapter.
#[cfg(any(
    all(feature = "vial", not(feature = "_no_usb")),
    all(feature = "vial", feature = "_ble"),
    all(feature = "rynk", feature = "_ble"),
))]
pub(crate) mod transport;
#[cfg(feature = "vial")]
pub(crate) mod via;

/// The active host-protocol service. Resolves to [`via::VialService`]
/// under the `vial` feature and [`rynk::RynkService`] under `rynk` (the
/// two are mutually exclusive).
#[cfg(feature = "rynk")]
pub use rynk::RynkService as HostService;
/// UART-backed rynk transport helper.
#[cfg(feature = "rynk")]
pub use rynk::run_rynk_uart;
#[cfg(feature = "vial")]
pub use via::VialService as HostService;

/// Run one host-protocol session over a BLE connection. Resolves to the
/// rynk transport under `rynk` and the Vial transport under `vial`.
#[cfg(all(feature = "rynk", feature = "_ble"))]
pub(crate) use crate::ble::rynk::run_host_ble;
#[cfg(all(feature = "vial", feature = "_ble"))]
pub(crate) use crate::ble::vial::run_host_ble;
/// Build and run one host-protocol session over USB. Resolves to the rynk
/// CDC-ACM transport under `rynk` and the Vial HID transport under `vial`.
#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
pub(crate) use crate::usb::rynk::{build_host_usb, run_host_usb};
#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
pub(crate) use crate::usb::vial::{build_host_usb, run_host_usb};
