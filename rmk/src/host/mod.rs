#[cfg(all(feature = "vial", feature = "_ble"))]
pub(crate) mod ble;
pub(crate) mod context;
#[cfg(feature = "rynk")]
pub(crate) mod rynk;
#[cfg(feature = "storage")]
pub(crate) mod storage;
#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
pub(crate) mod usb;
#[cfg(feature = "vial")]
pub(crate) mod via;

/// The active host-protocol service. Resolves to [`via::VialService`]
/// under the `vial` feature and [`rynk::RynkService`] under `rynk` (the
/// two are mutually exclusive). Construct with
/// `HostService::new(&keymap, &rmk_config)` and attach to USB transports
/// via `.with_host_service(&host_service)`.
#[cfg(feature = "rynk")]
pub use rynk::RynkService as HostService;
/// UART-backed rynk transport helper. Users with a free UART peripheral can
/// expose rynk over a serial line by calling [`run_rynk_uart`] with the
/// two halves of any [`embedded_io_async::Read`]/[`Write`]-implementing
/// pair (e.g. `embassy_*::usart::Uart::split()`).
#[cfg(feature = "rynk")]
pub use rynk::run_rynk_uart;
#[cfg(feature = "vial")]
pub use via::VialService as HostService;
