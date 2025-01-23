pub(crate) mod descriptor;
pub(crate) mod device_info;

#[cfg(feature = "_esp_ble")]
pub mod esp;
#[cfg(feature = "_nrf_ble")]
pub mod nrf;

#[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
pub use nrf::SOFTWARE_VBUS;
