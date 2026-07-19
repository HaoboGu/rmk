//! Rynk command handlers.

use rmk_types::protocol::rynk::endpoint::Endpoint;
use rmk_types::protocol::rynk::{RynkError, RynkMessage};

mod behavior;
mod bulk;
mod combo;
mod connection;
mod fork;
mod keymap;
mod layout;
mod macro_data;
mod morse;
mod status;
mod system;

/// Fixed-size endpoints: a request → response function. The [`Serve`] blanket
/// impl adds the decode → handle → encode wire glue.
pub(super) trait Handle<E: Endpoint> {
    async fn handle(&self, req: E::Request) -> Result<E::Response, RynkError>;
}

/// Bulk endpoints stream a page straight through the session buffer, so no `Vec`
/// is ever materialized. Implemented instead of [`Handle`].
pub(super) trait HandleBulk<E: Endpoint> {
    async fn handle_bulk(&self, msg: &mut RynkMessage<'_>) -> Result<(), RynkError>;
}

/// Dispatch-mode markers so the two [`Serve`] blanket impls don't overlap; bounds
/// alone can't disambiguate blanket impls. Inferred as `_`, never named.
pub(super) struct Fixed;
pub(super) struct Bulk;

/// The uniform surface the dispatcher calls, blanket-derived from [`Handle`]
/// (`Fixed`) or [`HandleBulk`] (`Bulk`). Handlers never implement it directly.
pub(super) trait Serve<E: Endpoint, Mode> {
    async fn serve(&self, msg: &mut RynkMessage<'_>) -> Result<(), RynkError>;
}

impl<E: Endpoint, T: Handle<E>> Serve<E, Fixed> for T {
    async fn serve(&self, msg: &mut RynkMessage<'_>) -> Result<(), RynkError> {
        let req = msg.decode_request::<E::Request>()?;
        let resp = self.handle(req).await?;
        msg.encode_response(&resp)
    }
}

impl<E: Endpoint, T: HandleBulk<E>> Serve<E, Bulk> for T {
    async fn serve(&self, msg: &mut RynkMessage<'_>) -> Result<(), RynkError> {
        self.handle_bulk(msg).await
    }
}
