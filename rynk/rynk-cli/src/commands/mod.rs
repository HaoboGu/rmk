//! Subcommand handlers.
//!
//! Each module has a single `run` entry, takes `&mut Client<T>` plus its
//! parsed args, and returns `anyhow::Result<()>`. Output goes to stdout
//! (data) / stderr (status). Nothing here re-implements protocol logic —
//! all of that lives in the `rynk` client.

pub mod bootloader;
pub mod caps;
pub mod get_key;
pub mod info;
pub mod layer;
pub mod led;
pub mod matrix;
pub mod reboot;
pub mod sleep;
pub mod wpm;
