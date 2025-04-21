use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::event::{Event, KeyEvent};

#[cfg(feature = "_ble")]
pub mod ble;
pub mod central;
/// Common abstraction layer of split driver
pub(crate) mod driver;
pub mod peripheral;
#[cfg(feature = "rp2040_pio")]
pub mod rp;
#[cfg(not(feature = "_ble"))]
pub mod serial;

/// Maximum size of a split message
pub const SPLIT_MESSAGE_MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE + 4;

/// Message used from central & peripheral communication
#[repr(u8)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum SplitMessage {
    Key(KeyEvent),
    /// Event from peripheral to central
    Event(Event),
    /// Led state, on/off, from central to peripheral
    LedState(bool),
    /// The central connection state, true if central has been connected to host.
    /// This message is sync from central to peripheral
    ConnectionState(bool),
    /// BLE Address, used in syncing address between central and peripheral
    Address([u8; 6]),
    /// Clear the saved peer info
    ClearPeer,
}
