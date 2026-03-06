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

/// Maximum bitmap size: supports up to 240 keys (e.g., 10 rows × 24 cols).
/// Each row uses ceil(num_cols / 8) bytes. Host decodes using num_rows/num_cols
/// from DeviceCapabilities.
pub const MAX_MATRIX_BITMAP_SIZE: usize = 30;

/// Current matrix key-press state as a bitmap.
/// Bit ordering: row-major, bit 0 = col 0, bit 1 = col 1, etc.
/// Total meaningful bytes = num_rows * ceil(num_cols / 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MatrixState {
    pub pressed_bitmap: heapless::Vec<u8, MAX_MATRIX_BITMAP_SIZE>,
}

/// Split keyboard peripheral status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SplitStatus {
    pub num_peripherals: u8,
    pub connected_peripherals: u8,
}
