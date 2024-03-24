macro_rules! config_matrix_pins_esp {
    (io: $io:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
        {
            let mut output_pins = [$(Into::<AnyPin<Output<PushPull>>>::into($io.pins.$out_pin.into_push_pull_output())), +];
            let input_pins = [$($io.pins.$in_pin.into_pull_down_input().degrade()), +];
            output_pins.iter_mut().for_each(|p| {
                let _ = p.set_low();
            });
            (input_pins, output_pins)
        }
    };
}
