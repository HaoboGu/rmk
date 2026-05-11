//! System endpoints — handshake, reboot, factory reset.

use rmk_types::protocol::rynk::{Cmd, DeviceCapabilities, ProtocolVersion, RynkResult, StorageResetMode};

use crate::transport::{Transport, TransportError};

/// Read the firmware's protocol version.
pub async fn get_version<T: Transport>(t: &mut T) -> Result<ProtocolVersion, TransportError> {
    t.request::<(), ProtocolVersion>(Cmd::GetVersion, &()).await
}

/// Read the firmware's full capability set. Host gates every subsequent
/// call on the flags / limits returned here.
pub async fn get_capabilities<T: Transport>(t: &mut T) -> Result<DeviceCapabilities, TransportError> {
    t.request::<(), DeviceCapabilities>(Cmd::GetCapabilities, &()).await
}

/// Reboot the device. Returns immediately; the next request will fail
/// with `Disconnected` once the firmware actually resets.
pub async fn reboot<T: Transport>(t: &mut T) -> Result<(), TransportError> {
    t.request::<(), ()>(Cmd::Reboot, &()).await
}

/// Jump to the bootloader (DFU mode). Same disconnect caveat as
/// [`reboot`].
pub async fn bootloader_jump<T: Transport>(t: &mut T) -> Result<(), TransportError> {
    t.request::<(), ()>(Cmd::BootloaderJump, &()).await
}

/// Reset persistent storage. `mode` selects what to wipe.
pub async fn storage_reset<T: Transport>(t: &mut T, mode: StorageResetMode) -> Result<RynkResult, TransportError> {
    t.request::<StorageResetMode, RynkResult>(Cmd::StorageReset, &mode)
        .await
}
