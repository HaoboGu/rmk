//! Topic publishers for rynk.
//!
//! Each publisher future subscribes to an existing in-crate pubsub channel
//! (`crate::event::LayerChangeEvent`, WPM, connection, sleep, LED, battery,
//! BLE status) and pushes the corresponding topic declared in
//! `rmk_types::protocol::rmk::topics` by calling
//! `postcard_rpc::server::WireTx::send` on the shared `&Tx`.
//!
//! Because `WireTx::send` / `send_raw` take `&self`, publishers and the
//! endpoint dispatcher can share one `&Tx` without a wrapping mutex — any
//! interior mutability a specific transport needs (e.g. `UsbBulkTx`'s
//! `&mut Sender`) is the transport's own concern.
//!
//! Composed with the endpoint dispatcher via `futures::select_biased!`
//! inside `RynkService::run()`.
