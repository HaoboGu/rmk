//! System endpoints — handshake, reboot, factory reset.

use rmk_types::protocol::rynk::{Cmd, DeviceCapabilities, ProtocolVersion, StorageResetMode};

use crate::RynkResult;
use crate::transport::{Transport, TransportError};

/// Read the firmware's protocol version.
pub async fn get_version<T: Transport>(t: &mut T) -> Result<RynkResult<ProtocolVersion>, TransportError> {
    t.request::<(), RynkResult<ProtocolVersion>>(Cmd::GetVersion, &()).await
}

/// Read the firmware's full capability set. Host gates every subsequent
/// call on the flags / limits returned here.
pub async fn get_capabilities<T: Transport>(t: &mut T) -> Result<RynkResult<DeviceCapabilities>, TransportError> {
    t.request::<(), RynkResult<DeviceCapabilities>>(Cmd::GetCapabilities, &())
        .await
}

/// Reboot the device. Returns immediately; the next request will fail
/// with `Disconnected` once the firmware actually resets.
pub async fn reboot<T: Transport>(t: &mut T) -> Result<RynkResult, TransportError> {
    t.request::<(), RynkResult>(Cmd::Reboot, &()).await
}

/// Jump to the bootloader (DFU mode). Same disconnect caveat as
/// [`reboot`].
pub async fn bootloader_jump<T: Transport>(t: &mut T) -> Result<RynkResult, TransportError> {
    t.request::<(), RynkResult>(Cmd::BootloaderJump, &()).await
}

/// Reset persistent storage. `mode` selects what to wipe.
pub async fn storage_reset<T: Transport>(t: &mut T, mode: StorageResetMode) -> Result<RynkResult, TransportError> {
    t.request::<StorageResetMode, RynkResult>(Cmd::StorageReset, &mode)
        .await
}
