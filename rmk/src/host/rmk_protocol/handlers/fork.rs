//! Handlers for the `fork/*` endpoint group.

use postcard_rpc::header::VarHeader;
use rmk_types::fork::Fork;
#[cfg(not(feature = "storage"))]
use rmk_types::protocol::rmk::RmkError;
use rmk_types::protocol::rmk::{RmkResult, SetForkRequest};

use super::super::Ctx;

pub(crate) async fn get_fork(ctx: &mut Ctx<'_>, _hdr: VarHeader, idx: u8) -> Fork {
    ctx.keymap
        .with_forks(|forks| forks.get(idx as usize).cloned())
        .unwrap_or_default()
}

pub(crate) async fn set_fork(_ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetForkRequest) -> RmkResult {
    // KeyMap exposes only `with_forks` (immutable view). Persist via FLASH and
    // let the storage task update both flash and the in-memory copy.
    #[cfg(feature = "storage")]
    {
        crate::channel::FLASH_CHANNEL
            .send(crate::storage::FlashOperationMessage::Fork {
                idx: req.index,
                fork: req.config,
            })
            .await;
        Ok(())
    }
    #[cfg(not(feature = "storage"))]
    {
        let _ = req;
        Err(RmkError::BadState)
    }
}
