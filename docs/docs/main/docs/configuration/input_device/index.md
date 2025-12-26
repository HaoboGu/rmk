# Input devices

All input devices are defined in the `[input_device]` table. Currently supported input device types include:

- [Rotary Encoder (encoder)](./encoder)
- [Joystick (joystick)](./joystick.md)
- [PMW3610 Optical Mouse Sensor (pmw3610)](./pmw3610.md)

Please refer to the corresponding documentation for detailed configuration settings.

## Processor Chain Configuration

The `processor_chain` field in the `[input_device]` section allows you to specify the order in which processors handle events. This is useful when you have multiple processors and want to control their execution order.

### Configuration

```toml
[input_device]
processor_chain = [
    "pmw3610_trackpoint_processor",  # Process trackpoint first
    "joystick_thumbstick_processor",  # Then joystick
    "scroll_processor",               # Then custom scroll processor
    "battery_processor",              # Then battery updates
]
```

### Complete Example

Here's a full example showing how processor names are generated:

**keyboard.toml:**
```toml
# PMW3610 sensor with custom name
[[input_device.pmw3610]]
name = "trackball"
# ... other config

# Joystick with custom name
[[input_device.joystick]]
name = "thumbstick"
# ... other config

# BLE battery monitoring
[ble]
enabled = true
battery_adc_pin = "P0_04"

# Specify processor order
[input_device]
processor_chain = [
    "trackball_processor",           # PMW3610 (name="trackball")
    "joystick_processor_thumbstick", # Joystick (name="thumbstick")
    "scroll_processor",              # Custom processor
    "battery_processor",             # Battery monitoring
]
```

**src/main.rs:**
```rust
#[rmk::keyboard]
mod my_keyboard {
    // Custom processor - name matches function name
    #[processor]
    fn scroll_processor() -> ScrollWheelProcessor {
        ScrollWheelProcessor::new(&keymap)
    }
}
```

Generated processor names:
- `trackball_processor` ← from `name = "trackball"`
- `joystick_processor_thumbstick` ← from `name = "thumbstick"`
- `scroll_processor` ← from function name
- `battery_processor` ← auto-generated for battery

### Processor Names

Processor names are automatically generated based on your configuration. Here's how to determine available processor names:

#### Built-in Processors

RMK automatically creates processors for configured input devices:

| Device Type | Naming Rule | Example Configuration | Generated Name |
|-------------|-------------|----------------------|----------------|
| **PMW3610** | `{name}_processor` | `[[input_device.pmw3610]]`<br>`name = "trackball"` | `trackball_processor` |
| **PMW3610** (no name) | `pmw3610_{idx}_processor` | `[[input_device.pmw3610]]`<br>(first device, no name) | `pmw3610_0_processor` |
| **Joystick** | `joystick_processor_{name}` | `[[input_device.joystick]]`<br>`name = "left_stick"` | `joystick_processor_left_stick` |
| **Battery** | `battery_processor` | BLE battery ADC enabled | `battery_processor` |

::: tip How to find your processor names
If you use an invalid processor name in `processor_chain`, the compiler will show an error listing all available processors. This is the easiest way to discover processor names!
:::

#### Custom Processors

Custom processors use the **function name** from your Rust code:

```rust
#[processor]
fn scroll_processor() -> ScrollWheelProcessor {  // Name: "scroll_processor"
    ScrollWheelProcessor::new(&keymap)
}

#[processor]
fn my_custom_filter() -> MyFilter {  // Name: "my_custom_filter"
    MyFilter::new()
}
```

### Default Order

If `processor_chain` is not specified, processors run in this order:
1. All built-in processors (in TOML definition order)
2. All custom processors (in Rust code definition order)

### How It Works

Each processor in the chain receives events and can either:
- **Continue**: Pass the event (possibly modified) to the next processor
- **Stop**: Event is fully handled, chain stops

This allows you to create sophisticated event processing pipelines with filtering, transformation, and routing logic.

For more details on creating custom processors, see the [Input Device Features](../../features/input_device.md#custom-processors) documentation.

## Configuring Multiple Input Devices

You can define and configure any number of input devices. To add multiple instances of a device, simply repeat the device type sub-table in your configuration:

```toml
# Encoder 1
[[input_device.encoder]]

# Encoder 2
[[input_device.encoder]]

# Encoder ..

# JoyStick 1
[[input_device.joystick]]

# JoyStick 2
[[input_device.joystick]]

# JoyStick ..
```

## Input device in split keyboards

For split keyboard configurations, it is necessary to specify which part of the keyboard (the central or the peripheral) the input device is physically connected to.

For example, instead of using `[[input_device.encoder]]`, you should use:

- `[[split.central.input_device.encoder]]` to add an encoder to the central.
- `[[split.peripheral.input_device.encoder]]` to add an encoder to the peripheral.

::: note
If your keyboard has multiple peripherals, `[[split.peripheral.input_device.<device_type>]]` always refers to the input device on the nearest `[[split.peripheral]]`.
:::

The following is an example which shows how to organize input device configuration when there are multiple peripherals

```toml
[split]

# Split central
[split.central]

# Encoder 0 on the central
[[split.central.input_device.encoder]]

# Encoder 1 on the central
[[split.central.input_device.encoder]]

# Peripheral 0
[[split.peripheral]]

# Encoder 0 on periphreal 0
[[split.peripheral.input_device.encoder]]

# Encoder 1 on periphreal 0
[[split.peripheral.input_device.encoder]]

# Joystick 0 on periphreal 0
[[split.peripheral.input_device.joystick]]

# Peripheral 1
[[split.peripheral]]

# Encoder 0 on periphreal 1
[[split.peripheral.input_device.encoder]]

# Encoder 1 on periphreal 1
[[split.peripheral.input_device.encoder]]

```
