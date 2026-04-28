macro_rules! config_direct_pins_sifli {
    (peripherals: $p:ident, direct_pins: [$([$($pin:tt),+ $(,)?]),+ $(,)?]) => {
        {
            [
                $(
                    [ $( config_direct_pin_sifli!(@pin $p, $pin) ),+ ]
                ),+
            ]
        }
    };
}

macro_rules! config_direct_pin_sifli {
    (@pin $p:ident, _) => {
        None
    };
    (@pin $p:ident, $pin:ident) => {
        Some(Input::new($p.$pin, sifli_hal::gpio::Pull::Up))
    };
}
