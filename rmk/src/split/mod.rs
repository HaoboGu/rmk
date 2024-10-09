use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::keyboard::KeyEvent;

pub mod central;
/// Common abstraction layer of split driver
pub(crate) mod driver;
#[cfg(feature = "_nrf_ble")]
pub(crate) mod nrf;
pub mod peripheral;
#[cfg(not(feature = "_nrf_ble"))]
pub(crate) mod serial;

/// Maximum size of a split message
pub const SPLIT_MESSAGE_MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE + 4;

/// Message used from central & peripheral communication
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize, defmt::Format)]
#[repr(u8)]
pub(crate) enum SplitMessage {
    /// Activated key info (row, col, pressed), from peripheral to central
    Key(KeyEvent),
    /// Led state, on/off
    LedState(bool),
}

/// Message used for synchronization between central thread and peripheral receiver(both in central board)
#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum KeySyncMessage {
    /// Response of `SyncSignal`, sent key state matrix from peripheral monitor to main
    /// u8 is the number of sent key states
    StartSend(u16),
    /// Key state: (row, col, key_pressing_state)
    Key(u8, u8, bool),
}

/// Signal used for inform that the matrix starts receives key states from peripheral key receiver
#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum KeySyncSignal {
    Start,
}
