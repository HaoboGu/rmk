//! Connection and status protocol types.

use serde::{Deserialize, Serialize};

use crate::connection::ConnectionType;

/// Current connection information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionInfo {
    pub connection_type: ConnectionType,
    pub ble_profile: u8,
    pub ble_connected: bool,
}

/// Current matrix key-press state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MatrixState {
    pub num_pressed: u8,
}

/// Split keyboard peripheral status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SplitStatus {
    pub num_peripherals: u8,
    pub connected_peripherals: u8,
}
