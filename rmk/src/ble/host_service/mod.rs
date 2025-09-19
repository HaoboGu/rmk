use embassy_sync::channel::Channel;

use crate::{RawMutex, VIAL_CHANNEL_SIZE};

#[cfg(feature = "vial")]
pub(crate) mod vial;

pub(crate) use vial::{BleVialServer as BleHostServer, VialService as HostService};

/// Channel for reading data from host GUI
pub(crate) static HOST_GUI_INPUT_CHANNEL: Channel<RawMutex, [u8; 32], VIAL_CHANNEL_SIZE> = Channel::new();
