//! Runtime-free Rynk host protocol client.
//!
//! [`Client`] drives the Rynk protocol over any [`Transport`] — a byte link to
//! a device. This crate does not open devices and does not depend on an async
//! runtime. Native, BLE, web, and third-party transports live in separate crates
//! that implement [`Transport`] and hand the link to [`Client::connect`].
//!
//! ```no_run
//! # use rmk_host::{Client, Transport};
//! # async fn run<T: Transport>(transport: T) -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = Client::connect(transport).await?;
//! let layer = client.get_current_layer().await?;
//! println!("active layer: {layer}");
//! # Ok(()) }
//! ```
//!
//! Each method returns the response value directly; a device rejection is
//! [`RequestError::Rejected`], so `?` carries both transport and firmware
//! failures.

pub mod client;
pub mod transport;

pub use client::{Client, ConnectError};
pub use rmk_types;
pub use transport::{MaybeSend, RequestError, TopicFrame, Transport, TransportError};

/// The wire types that appear in [`Client`] method signatures, re-exported for
/// downstream import. The full protocol crate is [`rmk_types`].
pub mod types {
    pub use rmk_types::action::{EncoderAction, KeyAction};
    pub use rmk_types::battery::BatteryStatus;
    pub use rmk_types::ble::BleStatus;
    pub use rmk_types::combo::Combo;
    pub use rmk_types::connection::{ConnectionStatus, ConnectionType};
    pub use rmk_types::fork::Fork;
    pub use rmk_types::led_indicator::LedIndicator;
    pub use rmk_types::morse::Morse;
    pub use rmk_types::protocol::rynk::{
        BehaviorConfig, Cmd, DeviceCapabilities, MacroData, MatrixState, PeripheralStatus, ProtocolVersion, RynkError,
        StorageResetMode,
    };
}
