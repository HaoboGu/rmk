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
//!
//!
#![no_std]
#![doc(html_root_url = "https://docs.rs/rmk-types/")]

/// Keyboard actions and behaviors.
///
/// This module defines the core action system used in RMK firmware.
/// Actions represent what happens when a key is pressed, from simple key
/// presses to complex behaviors like tap-hold, layer switching, and macros.
///
/// Key types:
/// - [`Action`](action::Action) - Single operations that keyboards send or execute
/// - [`KeyAction`](action::KeyAction) - Complex behaviors that keyboards should behave
/// - [`EncoderAction`](action::EncoderAction) - Rotary encoder actions
pub mod action;

/// Complete keycode definitions.
///
/// This module provides keycode definitions following the USB HID
/// specification, extended with additional codes
pub mod keycode;

/// LED indicator.
///
/// This module handles keyboard LED indicators such as Caps Lock, Num Lock,
/// and Scroll Lock. It provides efficient bitfield operations for these indicators.
pub mod led_indicator;

/// Modifier keys and their operations.
///
/// This module provides efficient handling of keyboard modifier states using
/// bitfield structures. It supports both left and right variants of all
/// standard modifiers (Ctrl, Shift, Alt, GUI).
pub mod modifier;

/// Mouse button state and operations.
///
/// This module handles mouse button combinations and states, supporting up to
/// 8 mouse buttons.
pub mod mouse_button;

/// Communication protocol definitions.
///
/// This module contains the protocol, type definitions and constants for communicating with
/// keyboard configuration software like Vial.
///
/// - [`vial`](protocol::vial) - Vial protocol implementation
pub mod protocol;
