use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal};
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

pub(crate) mod driver;
pub mod master;
pub mod slave;

/// Maximum size of a split message
pub const SPLIT_MESSAGE_MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE + 4;

/// Channels for synchronization between master and slave threads
const SYNC_SIGNAL_VALUE: Signal<CriticalSectionRawMutex, KeySyncSignal> = Signal::new();
pub(crate) static SYNC_SIGNALS: [Signal<CriticalSectionRawMutex, KeySyncSignal>; 4] =
    [SYNC_SIGNAL_VALUE; 4];
const SYNC_CHANNEL_VALUE: Channel<CriticalSectionRawMutex, KeySyncMessage, 8> = Channel::new();
pub(crate) static MASTER_SYNC_CHANNELS: [Channel<CriticalSectionRawMutex, KeySyncMessage, 8>; 4] =
    [SYNC_CHANNEL_VALUE; 4];

/// Message used from master & slave communication
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize, defmt::Format)]
#[repr(u8)]
pub enum SplitMessage {
    /// Activated key info (row, col, pressed), from slave to master
    Key(u8, u8, bool),
    /// Led state, on/off
    LedState(bool),
}

/// Message used for synchronization between master thread and slave receiver(both in master board)
#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum KeySyncMessage {
    /// Sent from master to slave thread, indicating master starts to read the key state matrix
    // StartRead,
    /// Response of `StartRead`, sent from slave to master, indicating that the slave starts to send the key state matrix.
    /// u8 is the number of sent key states
    StartSend(u16),
    /// Key state: (row, col, key_pressing_state)
    Key(u8, u8, bool),
}

/// Signal used for inform that the matrix starts receives key states from slave key receiver
#[derive(Debug, Clone, Copy, defmt::Format)]
pub(crate) enum KeySyncSignal {
    Start
}
