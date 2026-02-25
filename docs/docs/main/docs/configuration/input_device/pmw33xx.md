# PMW3360 / PMW3389 Optical Mouse Sensor

PMW3360 / PMW3389 are optical mouse sensors.

::: note
Both chips, the PMW3360 and PMW3389, are very similar. The main difference is the higher maximum cpi of the sensor. (12000 on the PMW3360 vs. 16000 on PMW3389)
They share one driver in RMK and the configuration of both is the same.

- PMW33xx uses full-duplex SPI. (MISO/ MOSI) Please note that because of the special requirements those sensors have for the switching of their chip select pin, they can not share an SPI bus with each other or any other SPI device. For each SPI peripheral (SPI0, SPI1 etc.) there can only be one sensor connected.
- Set `motion` pin for better power efficiency. If omitted, the sensor is polled.
- By default, report rate is limited to 125 Hz to prevent flooding the event channel, which causes latency issues especially over BLE.
- Only Nrf, RP2040 and STM32 are supported now.

:::

## `toml` configuration

```toml
[[input_device.pmw33xx]]
name = "trackball0"
sensor_type = "PMW3360" # or 3389
id = 0 # optional number between 0-255. Ids are used for debug prints. Set to 0 if omitted.

spi.instance = "SPI0"
spi.sck = "PIN_18"
spi.mosi = "PIN_19"
spi.miso = "PIN_16"
spi.cs = "PIN_17"
# By default this driver uses blocking SPI. For a small performance gain,
# you can define the DMA channels. Then the driver is using SPI with DMA.
# Figure out the DMA channel using the 'Peripherals' from the embassy hal crate
# (embassy-rp, embassy-stm32 etc.) similar to the PINs.
# On nrf52 the driver always uses DMA, the channels do not need to be specified.
# spi.tx_dma = "DMA_CH2" # omit for nrf52
# spi.rx_dma = "DMA_CH3" # omit for nrf52

# STM32 pins do not implement the Wait trait, therefore the motion pin has no effect.
# For STM32 you could use ExitInput and configure in Rust.
motion = "PIN_20" # Optional. If omitted, the sensor is polled.

# how often the sensor sends reports to the computer
report_hz = 125 # Optional: Report rate in Hz

cpi = 1600
rot_trans_angle = -15
liftoff_dist = 8
proc_invert_x = true
# proc_invert_y = true
# proc_swap_xy = true
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

Define a `PointingDevice` and add it to `run_all!` macro.
For a split keyboard this must be added to the file (`central.rs` or `peripheral.rs`) corresponding to the side the sensor is connected to.

::: Warning

For nrf52 chips you need to add an interrupt for the used SPI. For example when using SPI2:
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
         // res_cpi: 1600,
        // rot_trans_angle: 0,
        // liftoff_dist: 0x02,
        ..Default::default()
    };

    // Create the sensor device
    // for PMW3360
    const POINTING_DEV_ID: u8 = 0 // this ID can be anything form 0-255. Just make sure you don't use the same number twice for different sensors to avoid confusion.
    let mut PointingDevice::<Pmw33xx<_, _, _, Pmw3360Spec>>::new(POINTING_DEV_ID, spi_bus, cs, Some(motion), sensor_config);
    // for PMW3389
    let mut PointingDevice::<Pmw33xx<_, _, _, Pmw3389Spec>>::new(POINTING_DEV_ID, spi_bus, cs, Some(motion), sensor_config);

// There are several other initializers available to set polling and report rate.
// For example if you have an SROM for the sensor, you can upload it at startup using this:
// let mut pmw3360_device = PointingDevice::<Pmw33xx<_, _, _, Pmw3360Spec>>::new_with_firmware_poll_interval_report_hertz(
//     POINTING_DEV_ID,
//     spi_bus,
//     cs,
//     Some(motion)
//     sensor_config,
//     500, // poll interval
//     125, // report_hz
//     crate::pmw3360srom::PMW3360_SROM, // &[u8] in static memory (const)
// );
```

And define a `PointingProcessor` and add it to the `run_all!` macro to process the events.

::: warning

This should be added to the `central.rs`-File even if the sensor is on split peripheral.

:::

```rust
use rmk::input_device::pointing::{
    PointingProcessor, PointingProcessorConfig, PointingMode, ScrollConfig, SniperConfig
};

let pmw3360_proc_config = PointingProcessorConfig {
    device_id: 0,        // Match the id set on the PointingDevice (default 0)
    // invert_x: true,   // Invert X axis globally (all modes)
    // invert_y: true,   // Invert Y axis globally (all modes)
    // swap_xy: true,    // Swap X and Y axes globally
    ..Default::default()
};

let mut pmw3360_processor = PointingProcessor::new(&keymap, pmw3360_proc_config);

run_all!(pmw3360_processor, /* other processors and devices */)
```

## Per-Layer Pointing Modes

The `PointingProcessor` supports configuring different pointing behaviors for each layer. This is useful for:

- **Gaming**: Normal cursor on layer 0, sniper mode on layer 1
- **Productivity**: Cursor on layer 0, scroll mode on layer 1 for document navigation
- **CAD/Design**: Different precision levels for different tasks

### Available Modes

- **Cursor mode** (default): Normal mouse movement
- **Scroll mode**: Movement becomes scroll wheel/pan
- **Sniper mode**: Precision mode with reduced sensitivity

### Example Configuration

```rust
use rmk::input_device::pointing::{
    PointingProcessor, PointingProcessorConfig, PointingMode, ScrollConfig, SniperConfig
};

let mut pointing_processor = PointingProcessor::new(&keymap, PointingProcessorConfig::default());

// Configure different modes for each layer
pointing_processor
    .set_layer_mode(0, PointingMode::Cursor)                    // Layer 0: Normal cursor
    .set_layer_mode(1, PointingMode::Scroll(ScrollConfig {
        divisor_x: 8,    // Pan sensitivity (higher = slower)
        divisor_y: 8,    // Wheel sensitivity (higher = slower)
        invert_x: false, // Set true to reverse horizontal pan direction
        invert_y: false, // Set true to reverse scroll wheel direction
    }))
    .set_layer_mode(2, PointingMode::Sniper(SniperConfig {
        divisor: 4,      // Precision divisor (higher = slower)
        invert_x: false, // Set true to reverse X movement in sniper mode
        invert_y: false, // Set true to reverse Y movement in sniper mode
    }));
```

### Mode Details

**Cursor Mode**
- Direct 1:1 mapping of sensor movement to cursor movement
- Best for general navigation and pointer control

**Scroll Mode**
- X-axis movement → horizontal pan
- Y-axis movement → vertical scroll wheel
- `divisor_x` / `divisor_y`: sensitivity per axis — higher = slower. **Set to `0` to disable that axis entirely** (e.g. `divisor_x: 0` disables panning)
- `invert_x`: reverses horizontal pan direction (independent of global `invert_x`)
- `invert_y`: reverses scroll wheel direction (independent of global `invert_y`)
- Recommended divisor values: 4–16 (default: 8)

**Sniper Mode**
- Reduces movement speed for precision aiming
- `divisor`: applies to both X and Y axes — higher = slower, more precise
- `invert_x` / `invert_y`: reverses movement per axis in sniper mode
- Recommended divisor values: 2–8 (default: 4)
- Useful for games, CAD, or detailed work

::: tip
Use momentary layer keys (`MO(n)`) in your keymap to temporarily activate different pointing modes. The motion accumulator automatically resets when switching layers to ensure smooth transitions.
:::

