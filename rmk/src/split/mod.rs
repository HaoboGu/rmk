use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal,
};
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

/// Common abstraction layer of split driver
pub(crate) mod driver;
pub mod master;
#[cfg(feature = "_nrf_ble")]
pub(crate) mod nrf;
#[cfg(not(feature = "_nrf_ble"))]
pub(crate) mod serial;
pub mod slave;

/// Maximum size of a split message
pub const SPLIT_MESSAGE_MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE + 4;

/// Channels for synchronization between master and slave threads
const SYNC_SIGNAL_VALUE: Signal<CriticalSectionRawMutex, KeySyncSignal> = Signal::new();
pub(crate) static SYNC_SIGNALS: [Signal<CriticalSectionRawMutex, KeySyncSignal>; 4] =
    [SYNC_SIGNAL_VALUE; 4];
pub(crate) static SCAN_SIGNAL: Signal<CriticalSectionRawMutex, KeySyncSignal> = SYNC_SIGNAL_VALUE;
const SYNC_CHANNEL_VALUE: Channel<CriticalSectionRawMutex, KeySyncMessage, 8> = Channel::new();
pub(crate) static MASTER_SYNC_CHANNELS: [Channel<CriticalSectionRawMutex, KeySyncMessage, 8>; 4] =
    [SYNC_CHANNEL_VALUE; 4];

/// Message used from master & slave communication
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize, defmt::Format)]
#[repr(u8)]
pub(crate) enum SplitMessage {
    /// Activated key info (row, col, pressed), from slave to master
    Key(u8, u8, bool),
    /// Led state, on/off
    LedState(bool),
}

/// Message used for synchronization between master thread and slave receiver(both in master board)
#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum KeySyncMessage {
    /// Response of `SyncSignal`, sent key state matrix from slave monitor to main
    /// u8 is the number of sent key states
    StartSend(u16),
    /// Key state: (row, col, key_pressing_state)
    Key(u8, u8, bool),
}

/// Signal used for inform that the matrix starts receives key states from slave key receiver
#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum KeySyncSignal {
    Start,
}
