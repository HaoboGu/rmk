//! Handlers for the `conn/*` endpoint group (non-BLE specific).

use postcard_rpc::header::VarHeader;
use rmk_types::connection::ConnectionType;
use rmk_types::protocol::rmk::{RmkError, RmkResult};

use super::super::Ctx;

pub(crate) async fn get_connection_type(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> ConnectionType {
    crate::state::active_transport().unwrap_or(ConnectionType::Usb)
}

pub(crate) async fn set_connection_type(_ctx: &mut Ctx<'_>, _hdr: VarHeader, ty: ConnectionType) -> RmkResult {
    // Reject transports the firmware wasn't compiled with — silently
    // persisting them would brick the keyboard until the user re-flashes.
    if !is_transport_supported(ty) {
        return Err(RmkError::InvalidParameter);
    }
    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL
        .send(crate::storage::FlashOperationMessage::ConnectionType(ty))
        .await;
    Ok(())
}

const fn is_transport_supported(ty: ConnectionType) -> bool {
    match ty {
        ConnectionType::Usb => !cfg!(feature = "_no_usb"),
        ConnectionType::Ble => cfg!(feature = "_ble"),
    }
}
