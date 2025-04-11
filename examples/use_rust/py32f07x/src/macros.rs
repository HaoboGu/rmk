macro_rules! config_matrix_pins_py {
    (peripherals: $p:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
        {
            let mut output_pins = [$(Output::new($p.$out_pin, py32_hal::gpio::Level::Low, py32_hal::gpio::Speed::VeryHigh)), +];
            let input_pins = [$(Input::new($p.$in_pin, py32_hal::gpio::Pull::Down)), +];
            output_pins.iter_mut().for_each(|p| {
                p.set_low();
            });
            (input_pins, output_pins)
        }
    };
}
