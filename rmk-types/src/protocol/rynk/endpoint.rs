//! The Rynk endpoint/topic contracts: the [`Endpoint`] and [`Topic`] traits
//! that bind a command to its payload types.
//!
//! These describe only the wire schema. Buffer sizing lives in
//! [`super::command`]: postcard is slice-driven, so `MaxSize` matters only to
//! the no-allocator firmware, which folds it there — the traits stay free of it.

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::command::Cmd;

/// A request/response endpoint: its `Cmd` plus both payload types.
pub trait Endpoint {
    const CMD: Cmd;
    type Request: Serialize + DeserializeOwned;
    type Response: Serialize + DeserializeOwned;
}

/// A topic (server → host push): its `Cmd` plus the bare payload type.
pub trait Topic {
    const CMD: Cmd;
    type Payload: Serialize + DeserializeOwned;
}
