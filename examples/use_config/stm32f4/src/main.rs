#![no_main]
#![no_std]

use rmk::macros::rmk_keyboard;

// Create and run your keyboard with a single macro: `rmk_keyboard`
#[rmk_keyboard]
mod keyboard {
    use embassy_stm32::time::Hertz;
    use embassy_stm32::Config;

    #[Override(chip_config)]
    fn config() -> Config {
        // RCC config
        // Tested on stm32f411 with 25MHZ HSE and stm32f407 with 8MHZ HSE
        // The commented rcc configuration code is for stm32f407
        let mut config = Config::default();
        {
            use embassy_stm32::rcc::*;
            config.rcc.hse = Some(Hse {
                // freq: Hertz(25_000_000),
                freq: Hertz(8_000_000),
                mode: HseMode::Oscillator,
            });
            config.rcc.pll_src = PllSource::HSE;
            config.rcc.pll = Some(Pll {
                // prediv: PllPreDiv::DIV25,
                prediv: PllPreDiv::DIV8,
                mul: PllMul::MUL192,
                divp: Some(PllPDiv::DIV2), // 25mhz / 25 * 192 / 2 = 96Mhz.
                divq: Some(PllQDiv::DIV4), // 25mhz / 25 * 192 / 4 = 48Mhz.
                divr: None,
            });
            config.rcc.ahb_pre = AHBPrescaler::DIV1;
            // config.rcc.apb1_pre = APBPrescaler::DIV2;
            // config.rcc.apb2_pre = APBPrescaler::DIV1;
            config.rcc.apb1_pre = APBPrescaler::DIV4;
            config.rcc.apb2_pre = APBPrescaler::DIV2;
            config.rcc.sys = Sysclk::PLL1_P;
        }
        config
    }
}
