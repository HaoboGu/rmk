macro_rules! config_matrix_pins_hpm {
    (peripherals: $p:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
        {
            let mut output_pins = [$(::hpm_hal::gpio::Output::new($p.$out_pin, ::hpm_hal::gpio::Level::Low, ::hpm_hal::gpio::Speed::default())), +];
            let input_pins = [$(::hpm_hal::gpio::Input::new($p.$in_pin, ::hpm_hal::gpio::Pull::Down)), +];
            output_pins.iter_mut().for_each(|p| {
                p.set_low();
            });
            (input_pins, output_pins)
        }
    };
}
