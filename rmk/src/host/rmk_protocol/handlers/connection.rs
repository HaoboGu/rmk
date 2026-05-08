//! Handlers for the `conn/*` endpoint group (non-BLE specific).

use postcard_rpc::header::VarHeader;
use rmk_types::connection::ConnectionType;
use rmk_types::protocol::rmk::RmkResult;

use super::super::Ctx;

pub(crate) async fn get_connection_type(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> ConnectionType {
    crate::state::active_transport().unwrap_or(ConnectionType::Usb)
}

pub(crate) async fn set_connection_type(_ctx: &mut Ctx<'_>, _hdr: VarHeader, ty: ConnectionType) -> RmkResult {
    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL
        .send(crate::storage::FlashOperationMessage::ConnectionType(ty))
        .await;
    let _ = ty;
    Ok(())
}
