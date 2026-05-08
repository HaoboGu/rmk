#[cfg(feature = "_ble")]
pub(crate) mod ble;
#[cfg(feature = "rmk_protocol")]
pub mod rmk_protocol;
#[cfg(feature = "storage")]
pub(crate) mod storage;
#[cfg(not(feature = "_no_usb"))]
pub(crate) mod usb;
#[cfg(feature = "vial")]
pub(crate) mod via;

#[cfg(feature = "vial")]
pub use via::VialService as HostService;
