//! Typed wrappers over the [`Transport`](super::Transport) trait.
//!
//! These re-export [`Cmd`]-grouped helper modules so callers can write
//! `client.get_key(...)` instead of `client.transport.request(Cmd::GetKeyAction, &pos)`.
//! Every wrapper just translates types — there's no caching, no retry,
//! no batching.

pub mod behavior;
pub mod combo;
pub mod connection;
pub mod fork;
pub mod keymap;
pub mod macro_data;
pub mod morse;
pub mod status;
pub mod system;
