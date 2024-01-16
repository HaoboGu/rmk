// DEPRECIATED
macro_rules! _config_matrix_pins {
    (input: [$($in_port:ident.$in_pin:ident), *], output: [$($out_port:ident.$out_pin:ident), +]) => {
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

macro_rules! config_matrix_pins_stm32 {
    (peripherals: $p:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
        {
            let mut output_pins = [$(Output::new($p.$out_pin, embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::VeryHigh).degrade()), +];
            let input_pins = [$(Input::new($p.$in_pin, embassy_stm32::gpio::Pull::Down).degrade()), +];
            output_pins.iter_mut().for_each(|p| {
                p.set_low();
            });
            (input_pins, output_pins)
        }
    };
}
