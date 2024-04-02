pub(crate) mod descriptor;
pub(crate) mod nrf;

#[cfg(feature = "nrf52840_ble")]
pub use nrf::SOFTWARE_VBUS;