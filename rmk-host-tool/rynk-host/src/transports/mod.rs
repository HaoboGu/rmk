//! Concrete [`Transport`](super::Transport) implementations.
//!
//! - [`usb::UsbBulkTransport`] — Talks to the firmware's vendor-class
//!   bulk endpoints via `nusb`. Filters devices by the well-known WinUSB
//!   device-interface GUID `{F5F5F5F5-1234-5678-9ABC-DEF012345678}`.
//! - [`ble::BleGattTransport`] — Talks to the firmware's Rynk GATT
//!   service (UUID `F5F50001-…`) via `btleplug`.

#[cfg(feature = "ble")]
pub mod ble;
#[cfg(feature = "usb")]
pub mod usb;

#[cfg(feature = "ble")]
pub use ble::BleGattTransport;
#[cfg(feature = "usb")]
pub use usb::UsbBulkTransport;
