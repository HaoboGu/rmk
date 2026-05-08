//! Handlers for the `status/*` endpoint group.

use heapless::Vec;
use postcard_rpc::header::VarHeader;
use rmk_types::protocol::rmk::MatrixState;

use super::super::Ctx;

pub(crate) async fn get_current_layer(ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> u8 {
    ctx.keymap.active_layer()
}

pub(crate) async fn get_matrix_state(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> MatrixState {
    // Matrix state tracking is gated on `host_security`, which v1 does not
    // pull in (lock deferred to v2 — plan §3.7). Return an empty bitmap so the
    // wire shape is preserved and hosts can still poll without erroring.
    MatrixState {
        pressed_bitmap: Vec::new(),
    }
}
