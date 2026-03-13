use embassy_sync::channel::Channel;

use crate::RawMutex;

#[cfg(feature = "vial")]
pub(crate) mod vial;

#[cfg(feature = "vial")]
pub(crate) use vial::{BleVialServer as BleHostServer, VialService as HostService};

/// Channel for reading data from host GUI
#[cfg(feature = "vial")]
pub(crate) static HOST_GUI_INPUT_CHANNEL: Channel<RawMutex, [u8; 32], { crate::VIAL_CHANNEL_SIZE }> = Channel::new();
