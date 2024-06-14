macro_rules! config_output_pins_stm32 {
    (peripherals: $p:ident,  output: [$($out_pin:ident), +]) => {
        {
            let mut output_pins = [$(Output::new($p.$out_pin, embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::VeryHigh)), +];
            output_pins.iter_mut().for_each(|p| {
                p.set_low();
            });
            output_pins
        }
    };
}
