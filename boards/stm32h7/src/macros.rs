macro_rules! config_matrix_pins {
    (input: [$($in_port:ident.$in_pin:ident), *], output: [$($out_port: ident.$out_pin: ident), +]) => {
        {
            $(
                let $in_pin = $in_port.$in_pin.into_pull_down_input().erase();
            )*
            $(
                let mut $out_pin = $out_port.$out_pin.into_push_pull_output().erase();
            )+
            $(
                $out_pin.set_low();
            )+
            let output_pins = [$($out_pin), +];
            let input_pins = [$($in_pin), +];
            (input_pins, output_pins)
        }
    };
}

