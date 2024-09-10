#![no_main]
#![no_std]

mod keymap;
mod vial;
use rmk::macros::rmk_central;

use crate::keymap::KEYMAP;
use crate::vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

#[rmk_central]
mod keybaord_central {}
