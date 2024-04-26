#![no_std]
#![no_main]

mod keymap;
mod vial;

use crate::keymap::KEYMAP;
use rmk::macros::rmk_keyboard;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

#[rmk_keyboard]
mod keyboard {}
