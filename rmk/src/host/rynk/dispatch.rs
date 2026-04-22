//! Endpoint handler functions for rynk.
//!
//! One `async fn` per endpoint in `rmk_types::protocol::rmk::ENDPOINT_LIST`.
//! Handlers take `&RynkService<'_, _, _>` and the endpoint's request type, and
//! return its response type. Write handlers send `FlashOperationMessage`
//! to `crate::channel::FLASH_CHANNEL`, matching the behavior of
//! `rmk/src/host/via/mod.rs::process_via_packet` for equivalent commands.
//!
//! Handlers are currently `todo!()` stubs — the ICD is finalized in
//! `rmk-types`, so filling these in is mechanical and happens in follow-ups.
