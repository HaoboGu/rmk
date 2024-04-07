macro_rules! config_matrix_pins_esp {
    (peripherals: $p:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
        {
            let mut output_pins = [$(PinDriver::output($p.pins.$out_pin.downgrade_output()).unwrap()), +];
            let input_pins = [$(PinDriver::input($p.pins.$in_pin.downgrade_input()).unwrap()), +];
            output_pins.iter_mut().for_each(|p| {
                let _ = p.set_low();
            });
            (input_pins, output_pins)
        }
    };
}
