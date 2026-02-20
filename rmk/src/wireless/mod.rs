//! Wireless communication module for RMK
//!
//! This module provides abstractions for wireless keyboard communication,
//! including 2.4GHz protocols like Nordic Gazell.
//!
//! # Features
//!
//! - Transport-agnostic interface via `WirelessTransport` trait
//! - Multi-device addressing and management
//! - Nordic Gazell protocol implementation (nRF52840)
//! - Mock transport for testing
//! - Configurable parameters (channel, data rate, power)
//!
//! # Example
//!
//! ```no_run
//! use rmk::wireless::{WirelessTransport, GazellTransport, GazellConfig};
//!
//! // Create Gazell transport
//! let config = GazellConfig::low_latency();
//! let mut transport = GazellTransport::new(config);
//!
//! // Send an Elink frame
//! let frame = [0xAA, 0xBB, 0xCC];
//! transport.send_frame(&frame)?;
//!
//! // Receive frames
//! if let Some(received) = transport.recv_frame()? {
//!     // Process frame
//! }
//! ```
//!
//! # Multi-device Support
//!
//! ```no_run
//! use rmk::wireless::{DeviceManager, DeviceAddress};
//!
//! let mut manager = DeviceManager::new();
//! manager.register_device(DeviceAddress::new(0x1234, 0));
//! manager.update_device(0x1234, 1000, Some(-50));
//! ```

pub mod config;
pub mod device;
pub mod transport;

// Gazell module is always available (uses mock when wireless_gazell feature is disabled)
pub mod gazell;

#[cfg(test)]
pub mod mock;

// Re-export commonly used types
pub use config::{GazellConfig, WirelessConfig};
pub use device::{
    ConnectedDevice, DeviceAddress, DeviceManager, DeviceState, MultiDeviceFrame, MAX_DEVICES,
};
pub use transport::{WirelessError, WirelessTransport};

// GazellTransport is always exported (uses mock when wireless_gazell feature is disabled)
pub use gazell::GazellTransport;

#[cfg(test)]
pub use mock::{MockTransport, MockTransportPair};
