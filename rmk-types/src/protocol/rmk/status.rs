//! Connection and status protocol types.

use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::battery::BatteryStatus;

/// Maximum bitmap size: supports up to 240 keys (e.g., 10 rows × 24 cols).
/// Each row uses ceil(num_cols / 8) bytes. Host decodes using num_rows/num_cols
/// from DeviceCapabilities.
pub const MAX_MATRIX_BITMAP_SIZE: usize = 30;

/// Current matrix key-press state as a bitmap.
/// Bit ordering: row-major, bit 0 = col 0, bit 1 = col 1, etc.
/// Total meaningful bytes = num_rows * ceil(num_cols / 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MatrixState {
    pub pressed_bitmap: Vec<u8, MAX_MATRIX_BITMAP_SIZE>,
}

/// Status of a single split peripheral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeripheralStatus {
    pub connected: bool,
    pub battery: BatteryStatus,
}

/// Split keyboard status with per-peripheral detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SplitStatus {
    pub peripherals: Vec<PeripheralStatus, { crate::constants::SPLIT_PERIPHERALS_NUM }>,
}
