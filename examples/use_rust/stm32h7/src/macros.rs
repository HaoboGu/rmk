// macro_rules! config_matrix_pins_stm32 {
//     (peripherals: $p:ident, input: [$($in_pin:ident), *], output: [$($out_pin:ident), +]) => {
//         {
//             let mut output_pins = [$(Output::new($p.$out_pin, embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::VeryHigh).degrade()), +];
//             let input_pins = [$(Input::new($p.$in_pin, embassy_stm32::gpio::Pull::Down).degrade()), +];
//             output_pins.iter_mut().for_each(|p| {
//                 p.set_low();
//             });
//             (input_pins, output_pins)
//         }
//     };
// }

// macro_rules! output_pin_stm32 {
//     (peripherals: $p:ident, output: $out_pin:ident, initial_level: $initial_level: ident) => {{
//         let output_pin = Output::new(
//             $p.$out_pin,
//             embassy_stm32::gpio::Level::$initial_level,
//             embassy_stm32::gpio::Speed::VeryHigh,
//         )
//         .degrade();
//         Some(output_pin)
//     }};
// }
