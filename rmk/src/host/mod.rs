//! Host configurator support (keymap editing, firmware introspection, etc.).
//!
//! Currently implemented as Via/Vial over fixed 32-byte HID reports (USB + BLE).
//! Call sites import the active protocol's service as [`HostServiceImpl`].
//! The type implements [`crate::input_device::Runnable`], which is the bound
//! `run_keyboard` takes.

#[cfg(all(feature = "host", not(feature = "vial")))]
compile_error!("Enabling the `host` feature requires selecting a protocol: enable `vial`.");

#[cfg(feature = "storage")]
pub(crate) mod storage;
#[cfg(feature = "vial")]
pub(crate) mod via;

#[cfg(feature = "vial")]
pub(crate) use via::VialService as HostServiceImpl;
