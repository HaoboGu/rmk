//! Runnable trait implementation generation.
//!
//! This module handles the generation of `Runnable` trait implementations
//! for structs that combine input_device and processor behaviors.

mod generator;
mod naming;
mod subscriber;

pub use generator::generate_runnable;
pub use subscriber::generate_event_enum_and_dispatch;
