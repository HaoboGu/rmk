pub(crate) mod descriptor;

#[cfg(feature = "nrf_ble")]
pub(crate) mod nrf;
#[cfg(feature = "esp32_ble")]
pub mod esp;

#[cfg(feature = "nrf52840_ble")]
pub use nrf::SOFTWARE_VBUS;