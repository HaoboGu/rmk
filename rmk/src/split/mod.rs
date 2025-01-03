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
pub const SPLIT_MESSAGE_MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE + 6;

/// Message used from central & peripheral communication
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize, defmt::Format)]
#[repr(u8)]
pub(crate) enum SplitMessage {
    /// Key event from peripheral to central
    Key(KeyEvent),
    /// Led state, on/off, from central to peripheral
    LedState(bool),
    /// The central connection state, true if central has been connected to host.
    /// This message is sync from central to peripheral
    ConnectionState(bool),
}
