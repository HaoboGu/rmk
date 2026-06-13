//! The Rynk endpoint/topic contracts: the [`Endpoint`] and [`Topic`] traits
//! that bind a command to its payload types.

use postcard::experimental::max_size::MaxSize;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::RynkError;
use super::command::Cmd;

/// `const fn` max used by the compile-time payload-size folds (here for the
/// trait `MAX_PAYLOAD` defaults; in [`super::command`] for the table folds).
pub(crate) const fn max_const(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

/// A request/response endpoint: its `Cmd` plus both payload types.
pub trait Endpoint {
    const CMD: Cmd;
    type Request: Serialize + DeserializeOwned + MaxSize;
    type Response: Serialize + DeserializeOwned + MaxSize;
    /// Largest payload this endpoint puts on the wire in either direction.
    const MAX_PAYLOAD: usize = max_const(
        <Self::Request as MaxSize>::POSTCARD_MAX_SIZE,
        <Result<Self::Response, RynkError> as MaxSize>::POSTCARD_MAX_SIZE,
    );
}

/// A topic (server → host push): its `Cmd` plus the bare payload type.
pub trait Topic {
    const CMD: Cmd;
    type Payload: Serialize + DeserializeOwned + MaxSize;
    /// Largest payload this topic pushes.
    const MAX_PAYLOAD: usize = <Self::Payload as MaxSize>::POSTCARD_MAX_SIZE;
}
