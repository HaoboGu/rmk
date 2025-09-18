use crate::{RawMutex, VIAL_CHANNEL_SIZE};
use embassy_sync::channel::Channel;

#[cfg(feature = "vial")]
pub(crate) mod vial;

pub(crate) use vial::BleVialServer as BleHostServer;
pub(crate) use vial::VialService as HostService;

/// Channel for reading data from host GUI
pub(crate) static HOST_GUI_INPUT_CHANNEL: Channel<RawMutex, [u8; 32], VIAL_CHANNEL_SIZE> = Channel::new();
