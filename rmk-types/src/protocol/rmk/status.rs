//! Runtime status endpoint types.

use heapless::Vec;
use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// Maximum bitmap size: supports up to 256 keys (e.g., 16 rows × 16 cols).
/// Each row uses ceil(num_cols / 8) bytes. Host decodes using num_rows/num_cols
/// from DeviceCapabilities.
pub const PROTOCOL_MAX_MATRIX_BITMAP: usize = 32;

/// Current matrix key-press state as a bitmap.
/// Bit ordering: row-major, bit 0 = col 0, bit 1 = col 1, etc.
/// Total meaningful bytes = num_rows * ceil(num_cols / 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MatrixState {
    pub pressed_bitmap: Vec<u8, PROTOCOL_MAX_MATRIX_BITMAP>,
}

impl MaxSize for MatrixState {
    const POSTCARD_MAX_SIZE: usize =
        u8::POSTCARD_MAX_SIZE * PROTOCOL_MAX_MATRIX_BITMAP + crate::varint_max_size(PROTOCOL_MAX_MATRIX_BITMAP);
}

/// Status of a single split peripheral.
#[cfg(all(feature = "_ble", feature = "split"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeripheralStatus {
    pub connected: bool,
    pub battery: crate::battery::BatteryStatus,
}
