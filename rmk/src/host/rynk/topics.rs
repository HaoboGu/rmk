//! Topic publishers for rynk.
//!
//! Each publisher future subscribes to an existing in-crate pubsub channel
//! (`crate::event::LayerChangeEvent`, WPM, connection, sleep, LED, battery,
//! BLE status) and pushes the corresponding topic declared in
//! `rmk_types::protocol::rmk::topics` through a `postcard_rpc::server::Publisher`
//! sharing the same `WireTx` as the server.
//!
//! Composed with `server::run_server` via `futures::select_biased!` inside
//! `RynkService::run()`.
