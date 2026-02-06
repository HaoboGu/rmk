//! Runnable trait implementation generation.
//!
//! This module handles the generation of `Runnable` trait implementations
//! for structs that combine input_device, input_processor, and controller behaviors.

pub mod generator;
pub mod naming;
pub mod subscriber;

pub use generator::generate_runnable;
pub use naming::{
    event_type_to_handler_method_name, event_type_to_read_method_name, generate_unique_variant_names,
};
pub use subscriber::{generate_event_match_arms, generate_event_subscriber, EventTraitType};
