# Input Device Support

RMK's input device system provides a unified interface for managing input devices like rotary encoders, mouse sensors, joysticks, and other peripherals that generate input events.

## Overview

Input devices in RMK are hardware peripherals that generate input events. They provide:

- **Event-driven architecture** for efficient input handling
- **Processor chain** for flexible event transformation
- **Custom device/processor support** through attributes
- **Automatic split keyboard support**

## Architecture

### Event Flow

```
Input Devices → EVENT_CHANNEL → Processor Chain → HID Reports
```

**Key Point**: Devices and processors are completely decoupled through `EVENT_CHANNEL`. Devices generate events, processors transform them into HID reports.

### Core Traits

**InputDevice** - Generates events:
```rust
pub trait InputDevice {
    async fn read_event(&mut self) -> Event;
}
```

**InputProcessor** - Transforms events:
```rust
pub trait InputProcessor {
    async fn process(&mut self, event: Event) -> ProcessResult;
    fn get_keymap(&self) -> &RefCell<KeyMap>;
}
```

**ProcessResult** - Controls processor chain:
- `Continue(Event)` - Pass to next processor
- `Stop` - Event fully handled

## Built-in Devices

### Rotary Encoder
```toml
[[input_device.encoder]]
pin_a = "P0_29"
pin_b = "P0_28"
resolution = 4
```

### PMW3610 Mouse Sensor
```toml
[[input_device.pmw3610]]
name = "trackpoint"
spi.sck = "P0_12"
spi.mosi = "P0_13"
spi.miso = "P0_14"
cpi = 800
```

### Joystick
```toml
[[input_device.joystick]]
name = "thumbstick"
pin_x = "P0_04"
pin_y = "P0_05"
transform = [[1, 0], [0, 1]]
bias = [0, 0]
resolution = 16
```

See [configuration reference](../configuration/input_device) for all options.

## Custom Input Devices

Use `#[device]` to create custom devices. They have access to peripheral resources via `p`:

```rust
#[rmk_keyboard]
mod keyboard {
    #[device]
    fn trackball() -> Trackball {
        // Access I2C bus for trackball sensor via `p`
        Trackball::new(p.I2C0, p.P0_26, p.P0_27)
    }
}

// Pimoroni Trackball device
// I2C-based trackball with RGBW LEDs
pub struct Trackball {
    i2c: /* I2C type */,
    interrupt_pin: /* GPIO type */,
}

impl InputDevice for Trackball {
    async fn read_event(&mut self) -> Event {
        // Read X/Y movement and button state from I2C
        let (x, y, button) = self.read_i2c_registers().await;
        let mut data = [0u8; 16];
        data[0] = x;
        data[1] = y;
        data[2] = button;
        Event::Custom(data)
    }
}
```

**Key Points**:
- Access `p` for peripherals (GPIO, SPI, I2C, ADC, etc.)
- No TOML config needed - auto-discovered
- Implement `InputDevice` trait

## Custom Processors

Use `#[processor]` to create custom processors. They have access to `keymap`:

```rust
#[rmk_keyboard]
mod keyboard {
    #[processor]
    fn scroll_processor() -> ScrollWheelProcessor {
        ScrollWheelProcessor::new(&keymap)
    }
}

// Scroll wheel processor for trackball
// Converts Y-axis movement to scroll events when modifier key is held
pub struct ScrollWheelProcessor<'a, ...> {
    keymap: &'a RefCell<KeyMap<...>>,
    scroll_mode: bool,
}

impl InputProcessor for ScrollWheelProcessor {
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::Custom(data) => {
                let y_movement = data[1] as i8;

                // Check if scroll modifier (Fn key) is held
                let keymap = self.keymap.borrow();
                self.scroll_mode = keymap.is_key_pressed(SCROLL_MODIFIER_KEY);

                if self.scroll_mode && y_movement != 0 {
                    // Convert Y movement to scroll wheel
                    self.send_scroll_report(y_movement).await;
                    ProcessResult::Stop
                } else {
                    // Pass through as normal mouse movement
                    ProcessResult::Continue(event)
                }
            }
            _ => ProcessResult::Continue(event),
        }
    }

    fn get_keymap(&self) -> &RefCell<KeyMap> {
        self.keymap
    }
}
```

**Key Points**:
- Access `keymap` for layer/key state
- Return `Stop` when fully handled
- Return `Continue(event)` to pass to next

## Processor Chain

Control processor execution order in TOML:

```toml
[input_device]
processor_chain = [
    "pmw3610_trackpoint_processor",
    "scroll_processor",
    "battery_processor",
]
```

**Processor names**:
- Built-in: `{type}_{name}_processor` (e.g., `pmw3610_trackpoint_processor`)
- Custom: Function name from `#[processor]`

**Default order** (if not specified):
1. Built-in processors (TOML order)
2. Custom processors (code order)

## Split Keyboard Support

Configure devices per board:

```toml
[[split.central.input_device.encoder]]
pin_a = "P0_29"
pin_b = "P0_28"

[[split.peripheral.input_device.pmw3610]]
name = "peripheral_mouse"
# ... config ...
```

**Automatic behavior**:
- Device runs where configured
- Processor runs on central
- Events auto-routed from peripheral to central

## Best Practices

### Devices
- Use async operations (await events, don't busy-loop)
- Use `Event::Custom` for non-standard events
- Handle errors gracefully

### Processors
- Return `Stop` when fully handled
- Return `Continue(event)` otherwise
- Avoid blocking operations
- Use `send_report()` for HID reports

### Processor Order
- Put specialized processors first (filter early)
- Put fallback processors last
- Test your processor order

## Complete Example

```toml
[[input_device.encoder]]
pin_a = "P0_29"
pin_b = "P0_28"

[[input_device.pmw3610]]
name = "trackpoint"
spi.sck = "P0_12"
spi.mosi = "P0_13"
spi.miso = "P0_14"

[input_device]
processor_chain = [
    "pmw3610_trackpoint_processor",
    "scroll_processor",
]
```

```rust
#[rmk_keyboard]
mod keyboard {
    #[device]
    fn trackball() -> Trackball {
        Trackball::new(p.I2C0, p.P0_26, p.P0_27)
    }

    #[processor]
    fn scroll_processor() -> ScrollWheelProcessor {
        ScrollWheelProcessor::new(&keymap)
    }
}
```

## See Also

- [Input Device Configuration](../configuration/input_device) - TOML reference
- [Controller Support](./controller.md) - Output device architecture
