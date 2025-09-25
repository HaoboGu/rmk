macro_rules! config_matrix_pins_sifli {
    (peripherals: $p:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
        {
            let mut output_pins = [$(Output::new($p.$out_pin, sifli_hal::gpio::Level::Low)), +];
            let input_pins = [$(Input::new($p.$in_pin, sifli_hal::gpio::Pull::Down)), +];
            output_pins.iter_mut().for_each(|p| {
                p.set_low();
            });
            (input_pins, output_pins)
        }
    };
}
