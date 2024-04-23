#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use rmk::macros::rmk_keyboard;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
#[rmk_keyboard]
mod keyboard {}
