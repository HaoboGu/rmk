//! # RMK Types
//!
//! This crate provides fundamental type definitions and data structures used in RMK.
//!
//! ## Modules
//!
//! ### Core Modules
//! - [`action`] - Keyboard actions and behaviors (key presses, layer operations, macros)
//! - [`keycode`] - Keycode definitions including HID keycodes, media keys, and system control keycodes
//! - [`modifier`] - Modifier key combinations and operations
//! - [`mouse_button`] - Mouse button state and combinations
//! - [`led_indicator`] - LED indicator states and operations
//! - [`protocol`] - Communication protocol
//!
//!
//! ## Integration with RMK Ecosystem
//!
//! This crate is designed to work with other RMK components:
//!
//! - **rmk**: Core firmware logic uses these types for state management
//! - **rmk-config**: Configuration parsing produces these types
//! - **rmk-macro**: Code generation macros work with these type definitions

#![no_std]

pub mod action;
pub mod keycode;
pub mod led_indicator;
pub mod modifier;
pub mod mouse_button;
pub mod protocol;
