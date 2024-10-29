macro_rules! config_matrix_pins_rp {
    (peripherals: $p:ident, direct_pins: [$([$($pin:ident),+ $(,)?]),+ $(,)?]) => {
        {
            #[allow(unused_mut)]
            let mut pins = [
                $(
                    [
                        $(
                            Input::new(AnyPin::from($p.$pin), embassy_rp::gpio::Pull::Up)
                        ),+
                    ]
                ),+
            ];

            pins
        }
    };
}
