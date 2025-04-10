# Rotary encoders

## `toml` configurationn

You can define a rotary encoder in your `keyboard.toml`. The default type of rotary encoder is EC11, you can config it as:

```toml
[[input_device.encoder]]
pin_a = "P0_30"
pin_b = "P0_31"

# Whether to use the MCU's internal pull-up resistor, default to false
internal_pullup = false

# Phase is the working mode of the rotary encoders.
# Available mode: 
# - default: EC11 compatible, resolution = 1
# - e8h7: resolution = 2, reverse = true
# - resolution: customized resolution, the resolution value and reverse should be specified later
phase = "default" 

# The resolution represents how many pulses the encoder generates per detent.
# For examples, if your rotary encoder has 30 detents in total and generates 15 pulses per 360 degree rotation, then the resolution = 30/15 = 2.
# Number of detents and number of pulses can be found in your encoder's datasheet
resolution = 2

# Whether the direction of the rotary encoder is reversed.
reverse = false
```

Multiple encoders can be added directly, the encoder index is determined by the order:

```toml
# Encoder 0
[[input_device.encoder]]
pin_a = "P0_01"
pin_b = "P0_02"
phase = "default" 
# Encoder 1
[[input_device.encoder]]
pin_a = "P0_03"
pin_b = "P0_04"
phase = "default" 
```

## Rust configuration

With Rust, you can define a rotary encoder as the following:

```rust
    use rmk::input_device::rotary_encoder::RotaryEncoder;
    use rmk::input_device::rotary_encoder::DefaultPhase;
    let pin_a = Input::new(AnyPin::from(p.P1_06), embassy_nrf::gpio::Pull::None);
    let pin_b = Input::new(AnyPin::from(p.P1_04), embassy_nrf::gpio::Pull::None);
    let mut encoder = RotaryEncoder::with_phase(pin_a, pin_b, DefaultPhase, encoder_id);
```

You can also use the resolution based phase:

```rust
    use rmk::input_device::rotary_encoder::RotaryEncoder;
    let pin_a = Input::new(AnyPin::from(p.P1_06), embassy_nrf::gpio::Pull::None);
    let pin_b = Input::new(AnyPin::from(p.P1_04), embassy_nrf::gpio::Pull::None);
    // Create an encoder with resolution = 2, reversed = false
    let mut encoder = RotaryEncoder::with_resolution(pin_a, pin_b, 2, false, encoder_id)
```

After creating the rotary encoder device, a corresponding processor is also needed:

```rust
    use rmk::input_device::rotary_encoder::RotaryEncoderProcessor;
    let mut encoder_processor = RotaryEncoderProcessor::new(&keymap);
```

Lastly, add them to the finally runner:

```rust
    join4(
        run_devices! (
            (matrix, encoder) => EVENT_CHANNEL,
        ),
        run_processor_chain! {
            EVENT_CHANNEL => [encoder_processor],
        },
        keyboard.run(), // Keyboard is special
        run_rmk(&keymap, driver, storage, light_controller, rmk_config, sd),
    )
    .await;
```