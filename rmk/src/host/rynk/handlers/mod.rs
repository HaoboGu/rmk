//! Rynk command handlers.
//!
//! Each command row implements [`Handle`] on
//! [`RynkService`](super::RynkService), split across this directory by
//! domain. A handler is a pure request → response function; the trait's
//! provided [`handle_message`](Handle::handle_message) carries the shared
//! wire glue, so implementations never touch the wire view and cannot decode
//! or reply under the wrong `Cmd`.

use rmk_types::protocol::rynk::endpoint::Endpoint;
use rmk_types::protocol::rynk::{RynkError, RynkMessage};

pub(crate) mod behavior;
pub(crate) mod combo;
pub(crate) mod connection;
pub(crate) mod fork;
pub(crate) mod keymap;
pub(crate) mod macro_data;
pub(crate) mod morse;
pub(crate) mod status;
pub(crate) mod system;

/// One typed handler per command row: implementors define the bare
/// [`handle`](Self::handle) primitive; dispatch calls the provided
/// [`handle_message`](Self::handle_message) wrapper (the `Read::read` /
/// `read_exact` naming convention).
pub(super) trait Handle<E: Endpoint> {
    /// Compute the command's response — pure request → response logic, the
    /// wire never appears here.
    async fn handle(&self, req: E::Request) -> Result<E::Response, RynkError>;

    /// [`handle`](Self::handle) at the wire level, in place:
    /// decode`E::Request`, await the handler, and encode the reply envelope.
    async fn handle_message(&self, msg: &mut RynkMessage<'_>) -> Result<(), RynkError> {
        let req = msg.decode_request::<E::Request>()?;
        let resp = self.handle(req).await?;
        msg.encode_response(&resp)
    }
}
