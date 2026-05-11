//! Subcommand handlers.
//!
//! Each module has a single `run` entry, takes `&mut Client<T>` plus its
//! parsed args, and returns `anyhow::Result<()>`. Output goes to stdout
//! (data) / stderr (status). Nothing inside this module re-implements
//! protocol logic — all of that lives in `rynk_host`.

pub mod bootloader;
pub mod caps;
pub mod get_key;
pub mod info;
pub mod layer;
pub mod matrix;
pub mod reboot;
