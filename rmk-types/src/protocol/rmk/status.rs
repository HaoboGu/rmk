//! Status endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// Maximum bitmap size: supports up to 256 keys (e.g., 16 rows x 16 cols).
/// Each row uses ceil(num_cols / 8) bytes. Host decodes using num_rows/num_cols
/// from DeviceCapabilities.
pub const MATRIX_BITMAP_SIZE: usize = 32;

/// Current matrix key-press state as a bitmap.
/// Bit ordering: row-major, bit 0 = col 0, bit 1 = col 1, etc.
/// Total meaningful bytes = num_rows * ceil(num_cols / 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MatrixState {
    pub pressed_bitmap: heapless::Vec<u8, MATRIX_BITMAP_SIZE>,
}

impl MaxSize for MatrixState {
    const POSTCARD_MAX_SIZE: usize = MATRIX_BITMAP_SIZE + crate::varint_max_size(MATRIX_BITMAP_SIZE);
}

/// Status of a single split peripheral.
#[cfg(all(feature = "_ble", feature = "split"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PeripheralStatus {
    pub connected: bool,
    pub battery: crate::battery::BatteryStatus,
}

#[cfg(test)]
mod tests {
    use heapless::Vec;

    use super::*;
    use crate::protocol::rmk::test_utils::round_trip;

    #[test]
    fn round_trip_matrix_state() {
        let mut bitmap = Vec::new();
        bitmap.extend_from_slice(&[0b0000_0101, 0x00, 0b0010_0000]).unwrap();
        round_trip(&MatrixState { pressed_bitmap: bitmap });

        // Max-capacity case
        let mut bitmap = Vec::new();
        for i in 0..MATRIX_BITMAP_SIZE {
            bitmap.push(i as u8).unwrap();
        }
        round_trip(&MatrixState { pressed_bitmap: bitmap });
    }

    #[cfg(all(feature = "_ble", feature = "split"))]
    #[test]
    fn round_trip_peripheral_status() {
        use crate::battery::{BatteryStatus, ChargeState};
        round_trip(&PeripheralStatus {
            connected: true,
            battery: BatteryStatus::Available {
                charge_state: ChargeState::Discharging,
                level: Some(85),
            },
        });
        round_trip(&PeripheralStatus {
            connected: false,
            battery: BatteryStatus::Unavailable,
        });
    }
}
