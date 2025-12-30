use quote::{format_ident, quote};
use rmk_config::{ChipModel, ChipSeries, Pmw3360Config};

use super::Initializer;

/// Expand PMW3360 device configuration.
/// Returns (device initializers, processor initializers)
pub(crate) fn expand_pmw3360_device(
    pmw3360_config: Vec<Pmw3360Config>,
    chip: &ChipModel,
) -> (Vec<Initializer>, Vec<Initializer>) {
    if pmw3360_config.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // PMW3360 is only supported on nRF52, STM32 and RP2040
    match chip.series {
        // NOTE:; Nonblocking SPI with DMA is still marked unstable in esp_hal.
        ChipSeries::Nrf52 | ChipSeries::Rp2040 | ChipSeries::Stm32 => {}
        _ => {
            panic!("PMW3360 is only supported on nRF52, STM32, RP2350 and RP2040 chips");
        }
    }

    let mut device_initializers = vec![];
    let mut processor_initializers = vec![];

    for (idx, sensor) in pmw3360_config.iter().enumerate() {
        let sensor_name = if sensor.name.is_empty() {
            format!("pmw3360_{}", idx)
        } else {
            sensor.name.clone()
        };

        let device_ident = format_ident!("{}_device", sensor_name);
        let processor_ident = format_ident!("{}_processor", sensor_name);

        // Generate pin initialization
        let spi = &sensor.spi;

        let sck_ident = format_ident!("{}", spi.sck);
        let mosi_ident = format_ident!("{}", spi.mosi);
        let miso_ident = format_ident!("{}", spi.miso);
        let cs_ident = format_ident!("{}", spi.cs.as_ref().expect("pmw3360 requires `cs` in spi config"));
        let instance_ident = format_ident!("{}", spi.instance);

        let rx_dma_ident = match chip.series {
            ChipSeries::Nrf52 => {
                quote! {}
            }

            ChipSeries::Rp2040 | ChipSeries::Stm32 => {
                let rx_dma = spi
                    .rx_dma
                    .as_ref()
                    .expect("pmw3360 requires `rx_dma` in spi config");

                let rx_dma_ident = format_ident!("{}", rx_dma);

                quote! { #rx_dma_ident }
            }

            _ => unreachable!(),
        };
        let tx_dma_ident = match chip.series {
            ChipSeries::Nrf52 => {
                quote! {}
            }

            ChipSeries::Rp2040 | ChipSeries::Stm32 => {
                let tx_dma = spi
                    .tx_dma
                    .as_ref()
                    .expect("pmw3360 requires `tx_dma` in spi config");

                let tx_dma_ident = format_ident!("{}", tx_dma);

                quote! { #tx_dma_ident }
            }

            _ => unreachable!(),
        };

        // Generate config values
        let res_cpi: u16 = sensor.cpi.unwrap_or(1600);
        let rot_trans_angle: i8 = sensor.rot_trans_angle.unwrap_or(0);
        let liftoff_dist: u8 = sensor.liftoff_dist.unwrap_or(0);
        let invert_x = sensor.invert_x;
        let invert_y = sensor.invert_y;
        let swap_xy = sensor.swap_xy;

        // Generate motion pin initialization (optional)
        let motion_pin_init = if let Some(motion_pin) = &sensor.motion {
            let motion_ident = format_ident!("{}", motion_pin);
            match chip.series {
                ChipSeries::Nrf52 => quote! {
                    Some(::embassy_nrf::gpio::Input::new(p.#motion_ident, ::embassy_nrf::gpio::Pull::Up))
                },
                ChipSeries::Rp2040 => quote! {
                    Some(::embassy_rp::gpio::Input::new(p.#motion_ident, ::embassy_rp::gpio::Pull::Up))
                },
                ChipSeries::Stm32 => quote! {
                    Some(::embassy_stm32::gpio::Input::new(p.#motion_ident, ::embassy_stm32::gpio::Pull::Up))
                },
                _ => unreachable!(),
            }
        } else {
            match chip.series {
                ChipSeries::Nrf52 => quote! {
                    None::<::embassy_nrf::gpio::Input<'static>>
                },
                ChipSeries::Rp2040 => quote! {
                    None::<::embassy_rp::gpio::Input<'static>>
                },
                ChipSeries::Stm32 => quote! {
                    None::<::embassy_stm32::gpio::Input<'static>>
                },
                _ => unreachable!(),
            }
        };

        // Generate device initialization based on chip series
        let device_init = match chip.series {
            ChipSeries::Nrf52 => quote! {
                let mut #device_ident = {
                    use ::embassy_nrf::spim::{Spim, Config, MODE_3};
                    use ::embassy_nrf::gpio::{Output, Level, Pull};
                    use ::rmk::input_device::pmw3360::Pmw3360Config;
                    use ::rmk::input_device::pointing::PointingDevice;

                    let spi_inst = p.#instance_ident;
                    let sck = p.#sck_ident;
                    let mosi = p.#mosi_ident;
                    let miso = p.#miso_ident;
                    let cs = Output::new(p.#cs_ident, Level::High, OutputDrive::Standard);
                    let motion = #motion_pin_init;

                    let mut spi_config = Config::default();
                    spi_config.frequency = 2_000_000;
                    spi_config.mode = MODE_3;
                    let spi_bus = Spim::new(spi_inst, Irqs, sck, miso, mosi, spi_config);

                    let config = Pmw3360Config {
                        res_cpi: #res_cpi,
                        rot_trans_angle: #rot_trans_angle,
                        liftoff_dist: #liftoff_dist,
                        invert_x: #invert_x,
                        invert_y: #invert_y,
                        swap_xy: #swap_xy,
                        ..Default::default()
                    };

                    PointingDevice::new(spi_bus, cs, motion, config)
                };
            },
            ChipSeries::Rp2040 => quote! {
                let mut #device_ident = {
                    use ::embassy_rp::spi::{Spi, Config, Polarity, Phase};
                    use ::embassy_rp::gpio::{Output, Level, Pull};
                    use ::rmk::input_device::pmw3360::Pmw3360Config;
                    use ::rmk::input_device::pointing::PointingDevice;

                    let spi_inst = p.#instance_ident;
                    let sck = p.#sck_ident;
                    let mosi = p.#mosi_ident;
                    let miso = p.#miso_ident;
                    let cs = Output::new(p.#cs_ident, Level::High);
                    let tx_dma = p.#tx_dma_ident;
                    let rx_dma = p.#rx_dma_ident;
                    let motion = #motion_pin_init;

                    let mut spi_config = Config::default();
                    spi_config.polarity = Polarity::IdleHigh;
                    spi_config.phase = Phase::CaptureOnSecondTransition;
                    spi_config.frequency = 2_000_000;

                    let spi_bus = Spi::new(spi_inst, sck, mosi, miso, tx_dma, rx_dma, spi_config);

                    let config = Pmw3360Config {
                        res_cpi: #res_cpi,
                        rot_trans_angle: #rot_trans_angle,
                        liftoff_dist: #liftoff_dist,
                        invert_x: #invert_x,
                        invert_y: #invert_y,
                        swap_xy: #swap_xy,
                        ..Default::default()
                    };

                    PointingDevice::new(spi_bus, cs, motion, config)
                };
            },
            ChipSeries::Stm32 => quote! {
                let mut #device_ident = {
                    use ::embassy_stm32::spi::{Spi, Config, MODE_3};
                    use ::embassy_stm32::gpio::{Output, Level, Pull, Speed};
                    use ::embassy_stm32::time::Hertz;
                    use ::rmk::input_device::pmw3360::Pmw3360Config;
                    use ::rmk::input_device::pointing::PointingDevice;

                    let spi_inst = p.#instance_ident;
                    let sck = p.#sck_ident;
                    let mosi = p.#mosi_ident;
                    let miso = p.#miso_ident;
                    let cs = Output::new(p.#cs_ident, Level::High, Speed::Medium);
                    let tx_dma = p.#tx_dma_ident;
                    let rx_dma = p.#rx_dma_ident;
                    let motion = #motion_pin_init;

                    let mut spi_config = Config::default();
                    spi_config.frequency = Hertz::mhz(2);
                    spi_config.mode = MODE_3;

                    let spi_bus = Spi::new(spi_inst, sck, mosi, miso, tx_dma, rx_dma, spi_config);

                    let config = Pmw3360Config {
                        res_cpi: #res_cpi,
                        rot_trans_angle: #rot_trans_angle,
                        liftoff_dist: #liftoff_dist,
                        invert_x: #invert_x,
                        invert_y: #invert_y,
                        swap_xy: #swap_xy,
                        ..Default::default()
                    };

                    PointingDevice::new(spi_bus, cs, motion, config)
                };
            },
            _ => unreachable!(),
        };

        device_initializers.push(Initializer {
            initializer: device_init,
            var_name: device_ident,
        });

        // Generate processor initialization
        let processor_init = quote! {
            let mut #processor_ident = ::rmk::input_device::pointing::PointingProcessor::new(&keymap);
        };

        processor_initializers.push(Initializer {
            initializer: processor_init,
            var_name: processor_ident,
        });
    }

    (device_initializers, processor_initializers)
}
