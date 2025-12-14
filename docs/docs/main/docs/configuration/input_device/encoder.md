# Rotary encoders


A rotary encoder is a common input device that is used for volume control, page scrolling, and other functions.

It typically has two output pins (A and B) that generate quadrature signals as the encoder is rotated. By interpreting these signals, the direction and amount of rotation can be determined. Every time the encoder is rotated for some given amount, it generates a clockwise (CW) or counter-clockwise (CCW) event, which can be mapped to any key action like a normal key.

## `toml` configuration

### Define Rotary Encoders

You can define a rotary encoder in your `keyboard.toml`:

```toml
[[input_device.encoder]]
pin_a = "P0_30"
pin_b = "P0_31"

# Whether to use the MCU's internal pull-up resistor, default to false
internal_pullup = false

# Working mode of the encoder
# Available modes:
# - default: resolution = 1
# - resolution: custom resolution, requires specifying resolution and reverse parameters
phase = "resolution"

# `resolution` represents the number of steps generated per detent.
#
# When your encoder datasheet lists:
#   - detent = number of mechanical detents per full rotation
#   - pulse  = number of full quadrature cycles (A/B cycles) per full rotation
#
# Then the relationship is:
#   resolution = (pulse × 4) / detent
# because each full quadrature cycle (pulse) produces 4 edge transitions.
#
# For example — in the ALPS EC11E series (https://tech.alpsalpine.com/cms.media/product_catalog_ec_01_ec11e_en_611f078659.pdf):
#   detent = 30, pulse = 15 → resolution = (15 × 4) / 30 = 2
resolution = 2

# Or you can specify detent and pulse to calculate resolution automatically
resolution = { detent = 30, pulse = 15 }

# Whether the direction of the rotary encoder is reversed.
reverse = false
```

Multiple encoders can be added directly. Encoder indices are determined by the order they are defined.

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

### Define Encoder Actions

To define the actions triggered by encoder rotation, add a `encoders` field under each `[[layer]]` section, or add a `encoder_map` field under the `[layout]` section. The former method is preferred and the latter will be deprecated in the future.

A `encoders` field under `[[layer]]` is a 2D array where each entry corresponds to an encoder in the same order they are defined. Each entry is a 2-element array `[CW_action, CCW_action]`, representing the actions for clockwise and counter-clockwise rotations respectively for that encoder on that layer.

The `encoder_map` under `[layout]` is a 3D array where each entry is a 2D array as described above, representing the encoder actions for that layer.

**Structure**:
```toml
[[layers]]  # Layer 0
encoders = [[CW, CCW], [CW, CCW], ...]  # Encoder 0, encoder 1, ...

[[layers]]  # Layer 1
encoders = [[CW, CCW], [CW, CCW], ...]  # Encoder 0, encoder 1, ...

[[layers]]  # More layers...
encoders = [...]

# Alternatively, use encoder_map:

[layout]
encoder_map = [
  [ [CW, CCW], [CW, CCW], ... ],  # Layer 0: encoder 0, encoder 1, ...
  [ [CW, CCW], [CW, CCW], ... ],  # Layer 1: encoder 0, encoder 1, ...
  ...
]
```

**Example:**

```toml
# Define encoder actions:

# Layer 0:
#   - Encoder 0: CW -> AudioVolUp, CCW -> AudioVolDown
#   - Encoder 1: CW -> PageDown, CCW -> PageUp
# Layer 1:
#   - Encoder 0: No action ("_")
#   - Encoder 1: CW -> BrightnessUp, CCW -> BrightnessDown

[[layers]]  # Layer 0
# ... keys ...
encoders = [["AudioVolUp", "AudioVolDown"], ["PageDown", "PageUp"]]

[[layers]]  # Layer 1
# ... keys ...
encoders = [["_", "_"], ["BrightnessUp", "BrightnessDown"]]

# Alternatively, use encoder_map:

[layout]
rows = 5
cols = 4
layers = 2
# ... matrix_map ...
encoder_map = [
  [["AudioVolUp", "AudioVolDown"], ["PageDown", "PageUp"]],
  [["_", "_"], ["BrightnessUp", "BrightnessDown"]]
]
```

**Notes:**
- If the actions for an encoder are not specified in `encoders` or `encoder_map`, they will default to no action.
- The number of encoder entries should match the number of physical encoders defined in `[[input_device.encoder]]`.

## Rust configuration

With Rust, you can define a rotary encoder as the following:

```rust
    use rmk::input_device::rotary_encoder::RotaryEncoder;
    use rmk::input_device::rotary_encoder::DefaultPhase;
    let pin_a = Input::new(p.P1_06, embassy_nrf::gpio::Pull::None);
    let pin_b = Input::new(p.P1_04, embassy_nrf::gpio::Pull::None);
    let mut encoder = RotaryEncoder::with_phase(pin_a, pin_b, DefaultPhase, encoder_id);
```

You can also use the resolution based phase:

```rust
    use rmk::input_device::rotary_encoder::RotaryEncoder;
    let pin_a = Input::new(p.P1_06, embassy_nrf::gpio::Pull::None);
    let pin_b = Input::new(p.P1_04, embassy_nrf::gpio::Pull::None);
    // Create an encoder with resolution = 2, reversed = false
    let mut encoder = RotaryEncoder::with_resolution(pin_a, pin_b, 2, false, encoder_id)
```

Then add the encoder to the device list of `run_device`.

```rust
    join3(
        run_devices! (
            (matrix, encoder) => EVENT_CHANNEL,
        ),
        keyboard.run(), // Keyboard is special
        run_rmk(&keymap, driver, storage, rmk_config, sd),
    )
    .await;
```

Defining Encoder Actions in `keymap.rs`:

```rust
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
    ... // Standard keymap definition
    ]
}

pub const fn get_default_encoder_map() -> [[EncoderAction; NUM_ENCODER]; NUM_LAYER] {
    [
        // Layer 0
        [
            // Encoder 0: (Clockwise, Counter-Clockwise)
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)), 
            // Encoder 1:
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)), 
        ],
        ...
    ]
}
```
