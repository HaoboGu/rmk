use quote::{format_ident, quote};
use rmk_config::{ChipModel, ChipSeries, Iqs5xxConfig};

use super::Initializer;

/// Expand IQS5xx device configuration.
/// Returns (device initializers, processor initializers)
pub(crate) fn expand_iqs5xx_device(
    iqs5xx_config: Vec<Iqs5xxConfig>,
    chip: &ChipModel,
) -> (Vec<Initializer>, Vec<Initializer>) {
    if iqs5xx_config.is_empty() {
        return (Vec::new(), Vec::new());
    }

    match chip.series {
        ChipSeries::Rp2040 => {}
        _ => {
            panic!("IQS5xx is only supported on RP2040 chips");
        }
    }

    let mut device_initializers = Vec::new();
    let mut processor_initializers = Vec::new();

    for (idx, sensor) in iqs5xx_config.iter().enumerate() {
        let sensor_name = if sensor.name.is_empty() {
            format!("iqs5xx_{idx}")
        } else {
            sensor.name.clone()
        };

        let device_ident = format_ident!("{}_device", sensor_name);
        let processor_ident = format_ident!("{}_processor", sensor_name);

        let i2c = &sensor.i2c;
        let i2c_instance_ident = format_ident!("{}", i2c.instance);
        let sda_ident = format_ident!("{}", i2c.sda);
        let scl_ident = format_ident!("{}", i2c.scl);
        let rdy_ident = format_ident!("{}", sensor.rdy);
        let rst_ident = format_ident!("{}", sensor.rst);

        let address = i2c.address;
        let frequency_set = if let Some(freq) = i2c.frequency {
            quote! { i2c_config.frequency = #freq; }
        } else {
            quote! {}
        };

        let enable_single_tap = sensor.enable_single_tap;
        let enable_press_and_hold = sensor.enable_press_and_hold;
        let press_and_hold_time_ms = sensor.press_and_hold_time_ms;
        let enable_two_finger_tap = sensor.enable_two_finger_tap;
        let enable_scroll = sensor.enable_scroll;
        let invert_x = sensor.invert_x;
        let invert_y = sensor.invert_y;
        let swap_xy = sensor.swap_xy;
        let bottom_beta = sensor.bottom_beta;
        let stationary_threshold = sensor.stationary_threshold;
        let poll_interval_ms = sensor.poll_interval_ms;
        let scroll_divisor = sensor.scroll_divisor as i16;
        let natural_scroll_x = sensor.natural_scroll_x;
        let natural_scroll_y = sensor.natural_scroll_y;

        let device_init = match chip.series {
            ChipSeries::Rp2040 => quote! {
                let mut #device_ident = {
                    use ::embassy_rp::gpio::{Input, Output, Pull, Level};
                    use ::embassy_rp::i2c::{Config as I2cConfig, I2c};
                    use ::rmk::input_device::iqs5xx::{Iqs5xxConfig, Iqs5xxDevice};

                    let mut i2c_config = I2cConfig::default();
                    #frequency_set

                    let i2c = I2c::new_async(p.#i2c_instance_ident, p.#scl_ident, p.#sda_ident, Irqs, i2c_config);
                    let rdy = Input::new(p.#rdy_ident, Pull::Down);
                    let rst = Output::new(p.#rst_ident, Level::High);

                    let config = Iqs5xxConfig {
                        addr: #address,
                        enable_single_tap: #enable_single_tap,
                        enable_press_and_hold: #enable_press_and_hold,
                        press_and_hold_time_ms: #press_and_hold_time_ms,
                        enable_two_finger_tap: #enable_two_finger_tap,
                        enable_scroll: #enable_scroll,
                        invert_x: #invert_x,
                        invert_y: #invert_y,
                        swap_xy: #swap_xy,
                        bottom_beta: #bottom_beta,
                        stationary_threshold: #stationary_threshold,
                        ..Iqs5xxConfig::default()
                    };

                    Iqs5xxDevice::with_poll_interval(i2c, rdy, rst, config, #poll_interval_ms)
                };
            },
            _ => unreachable!(),
        };

        device_initializers.push(Initializer {
            initializer: device_init,
            var_name: device_ident,
        });

        let processor_init = quote! {
            let mut #processor_ident = ::rmk::input_device::iqs5xx::Iqs5xxProcessor::new(
                &keymap,
                ::rmk::input_device::iqs5xx::Iqs5xxProcessorConfig {
                    scroll_divisor: #scroll_divisor,
                    natural_scroll_x: #natural_scroll_x,
                    natural_scroll_y: #natural_scroll_y,
                }
            );
        };

        processor_initializers.push(Initializer {
            initializer: processor_init,
            var_name: processor_ident,
        });
    }

    (device_initializers, processor_initializers)
}
