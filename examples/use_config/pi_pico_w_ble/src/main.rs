#![no_main]
#![no_std]

use rmk::macros::rmk_keyboard;

// Create and run your keyboard with a single macro: `rmk_keyboard`, that's it!
#[rmk_keyboard]
mod keyboard {}
