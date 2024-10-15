#![no_main]
#![no_std]

mod vial;

use rmk::macros::rmk_keyboard;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

// Create and run your keyboard with a single macro: `rmk_keyboard`
#[rmk_keyboard]
mod keyboard {
    use embassy_stm32::{time::Hertz, Config};

    #[Override(chip_config)]
    fn config() -> Config {
        let mut config = Config::default();
        config.rcc.hse = Some(Hertz(8_000_000));
        config.rcc.sys_ck = Some(Hertz(48_000_000));
        config.rcc.pclk1 = Some(Hertz(24_000_000));
        config
    }
}
