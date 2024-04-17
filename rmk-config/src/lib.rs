//! This crate contains all configurations that you can customize RMK.
//!
//! There are two TYPES of configuration: [toml_config] and [keyboard_config].
//!
//! - [toml_config]: the configuration describles how the RMK toml configuration file looks like. It can be loaded directly from a toml file.
//!
//! > Why we have two configurations?
//!
//! > We want to provide a user-friendly representation of configurations, that's why [toml_config] exists.
//! For example, to define the keyboard matrix, users can just use a list of string in toml like: `["PA1", "PA2"]`.
//! This list could be automatically converted to an actual GPIO matrix associated to microncontroller in [keyboard_config].
//!
//! - [keyboard_config]: the configuration which is internally used in RMK.
//! [keyboard_config] is what RMK's code receives. You can safely ignore it unless you want to dive into the RMK source code.
#![cfg_attr(not(feature = "toml"), no_std)]

mod keyboard_config;
pub use keyboard_config::*;

#[cfg(feature = "toml")]
pub mod toml_config;
