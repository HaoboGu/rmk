# IQS5xx Trackpad

IQS5xx is a capacitive trackpad controller used in Azoteq TPS65 modules.

::: note

- IQS5xx uses I2C + RDY/RST GPIO.
- Only RP2040 is supported right now.

:::

## `toml` configuration

```toml
[[input_device.iqs5xx]]
name = "trackpad"

# I2C pins and address
[input_device.iqs5xx.i2c]
instance = "I2C0"
sda = "PIN_0"
scl = "PIN_1"
address = 0x74
frequency = 400000 # optional

# RDY/RST pins
rdy = "PIN_3"
rst = "PIN_2"

# Gesture settings (optional, defaults shown)
enable_single_tap = true
enable_press_and_hold = true
press_and_hold_time_ms = 250
enable_two_finger_tap = true
enable_scroll = true

# Axis settings (optional)
invert_x = false
invert_y = false
swap_xy = false

# Sensitivity (optional)
bottom_beta = 5
stationary_threshold = 5

# Polling interval and scroll scaling (optional)
poll_interval_ms = 5
scroll_divisor = 32
natural_scroll_x = false
natural_scroll_y = false
```

## Split keyboards

When using split keyboards, specify where the device is physically connected:

```toml
[[split.central.input_device.iqs5xx]]
name = "trackpad"
# ...same fields...
```

or

```toml
[[split.peripheral.input_device.iqs5xx]]
name = "trackpad"
# ...same fields...
```

## Rust configuration

If you are not using `keyboard.toml`, you can wire it manually:

```rust
use rmk::input_device::iqs5xx::{Iqs5xxConfig, Iqs5xxDevice, Iqs5xxProcessor, Iqs5xxProcessorConfig};

let config = Iqs5xxConfig {
    enable_scroll: true,
    ..Default::default()
};

let mut tp = Iqs5xxDevice::new(i2c, rdy_pin, rst_pin, config);
let mut tp_proc = Iqs5xxProcessor::new(&keymap, Iqs5xxProcessorConfig::default());

run_devices!((matrix, tp) => EVENT_CHANNEL);
run_processor_chain! { EVENT_CHANNEL => [tp_proc] };
```

::: warning

If the trackpad is on a split peripheral, `Iqs5xxProcessor` must run on the central.

:::
