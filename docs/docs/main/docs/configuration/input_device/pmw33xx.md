# PMW3360 / PMW3389 Optical Mouse Sensor

PMW3360 / PMW3389 are optical mouse sensors.

::: note
Both chips, the PMW3360 and PMW3389 are very similar. The main difference is the higher maximum cpi of the sensor. (12000 on the PMW3360 vs. 16000 on PMW3389)
They share one driver in RMK and the configuration of both is the same.

- PMW33xx uses full-duplex SPI. (MISO/ MOSI) Please note that because of the special requirements those sensors have for the switching of their chip select pin, they can not share an SPI bus with each other or any other SPI device. For each SPI peripheral (SPI0, SPI1 etc.) there can only be one sensor connected.
- `motion` pin is optional. If omitted, the sensor is polled.
- Only Nrf, RP2040 and STM32 are supported now.

:::

## `toml` configuration

```toml
[[input_device.pmw33xx]]
name = "trackball0"
sensor_type = "PMW3360" # or 3389

spi.instance = "SPI0"
spi.sck = "PIN_18"
spi.mosi = "PIN_19"
spi.miso = "PIN_16"
spi.cs = "PIN_17"
# By default this driver uses blocking SPI. For a small performance gain,
# you can define the DMA channels. Then the driver is using SPI with DMA.
# Figure out the DMA channel using the Peripherals from the embassy hal crate
# (embassy-rp, embassy-stm32 etc.) similar to the PINs.
# On nrf52 the driver alsways uses DMA, the channels do not need to be specified.
# spi.tx_dma = "DMA_CH2" # omit for nrf52
# spi.rx_dma = "DMA_CH3" # omit for nrf52

motion = "PIN_20" # Optional. If omitted, the sensor is polled.

cpi = 1600
rot_trans_angle = -15
liftoff_dist = 8
invert_x = true
# invert_y = true
# swap_xy = true
```

### Split

To add the sensor to the central or peripheral use 
```toml
[[split.central.input_device.pmw33xx]]
name = ...

# resp.
[[split.peripheral.input_device.pmw33xx]]
name = ...
```

## Rust configuration

Define a `PointingDevice` and add it to `run_devices!` macro.
For a split keyboard this must be added to the file (`central.rs` or `peripheral.rs`) corresponding to the side the sensor is connected to.

::: Warning

For nrf52 chips you need to add an interrupt for the used SPI. For expample when using SPI2:
```rust
use ::embassy_nrf::spim;

bind_interrupts!(struct Irqs {
    (...)
    SPI2 => spim::InterruptHandler<peripherals::SPI2>;
});
```

:::

```rust
    use embassy_rp::spi::{Spi, Config, Polarity, Phase};
    use embassy_rp::gpio::{Level, Output, Pull};
    use rmk::input_device::pointing::PointingDevice;
    // for PMW3360 import
    use rmk::input_device::pmw33xx::{Pmw33xx, Pmw33xxConfig, Pmw3360Spec};
    // for PMW3389 import
    use rmk::input_device::pmw33xx::{Pmw33xx, Pmw33xxConfig, Pmw3389Spec};

    let mut spi_cfg = Config::default();
    // // MODE_3 = Polarity::IdleHigh + Phase::CaptureOnSecondTransition
    spi_cfg.polarity = Polarity::IdleHigh;
    spi_cfg.phase = Phase::CaptureOnSecondTransition;
    spi_cfg.frequency = 2_000_000;

    // // Create GPIO pins
    let sck = p.PIN_18;
    let mosi = p.PIN_19;
    let miso = p.PIN_16;
    let cs = Output::new(p.PIN_17, Level::High);
    let motion = Input::new(p.PIN_20, Pull::Up);

    // Create the SPI bus
    let spi_bus = Spi::new(p.SPI0, sck, mosi, miso, p.DMA_CH2, p.DMA_CH3, spi_cfg);

    // Initialize PMW33xx mouse sensor
    let sensor_config = Pmw33xxConfig {
        res_cpi: 1600,
        rot_trans_angle: -15,
        liftoff_dist: 0x08,
        swap_xy: false,
        invert_x: true,
        invert_y: false,
        ..Default::default()
    };

    // Create the sensor device
    // for PMW3360
    let mut PointingDevice::<Pmw33xx<_, _, _, Pmw3360Spec>>::new(spi_bus, cs, Some(motion), sensor_config);
    // for PMW3389
    let mut PointingDevice::<Pmw33xx<_, _, _, Pmw3389Spec>>::new(spi_bus, cs, Some(motion), sensor_config);
```

And define a `PointingProcessor` and add it to the `run_processor_chain!` macro to process the events.

::: warning

This should be added to the `central.rs`-File even if the sensor is on split peripheral.

:::

```rust
    use rmk::input_device::pointing::PointingProcessor;

    let mut pmw3360_processor = PointingProcessor::new(&keymap);

    run_processor_chain! {
        EVENT_CHANNEL => [pmw3360_processor],
    },
```

