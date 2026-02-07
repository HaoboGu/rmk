# Input Device

RMK's input device system is a unified, event-driven framework designed to support diverse input devices.

::: info What are input devices?
Input devices are hardware components that provide input to the keyboard firmware, such as key matrix, rotary encoders, joysticks, pointing devices, ADC sensors, and more.
:::

## Event-driven input system

RMK uses an event-driven architecture for input handling. Input devices produce events, and input processors consume and process those events. Events flow through type-safe channels, decoupling producers from consumers.

The complete event chain in RMK is:

```
┌────────────┐
│input device│
└─────┬──────┘
      │ publish
      ▼
┌────────────┐
│input events│
└─────┬──────┘
      │ subscribe
      ▼
┌───────────────┐
│input processor│
└─────┬─────────┘
      │ publish
      ▼
┌─────────────────┐
│controller events│
└─────┬───────────┘
      │ subscribe
      ▼
┌────────────┐
│ controller │
└────────────┘
```

**Key concepts:**

- **Input Event** — A type-safe message carrying data from an input device (e.g., `KeyboardEvent`, `PointingEvent`, `BatteryAdcEvent`). Defined with the `#[input_event]` macro.
- **Input Device** — A hardware driver that reads from a peripheral and publishes input events. Defined with the `#[input_device]` macro.
- **Input Processor** — A component that subscribes to input events and processes them (e.g., converting raw ADC values to battery percentage). Defined with the `#[input_processor]` macro.
- **Runnable** — All input devices and processors implement the `Runnable` trait. Since this implementation is macro-generated, these components can be easily run concurrently using the `run_all!` macro.

## Input Event

Input events are the messages that flow from input devices to input processors. Each event type has its own dedicated channel, where it is published by an `InputDevice` and subscribed to by an `InputProcessor`.

### Defining input events

Use the `#[input_event]` macro to define a custom input event:

```rust
use rmk_macro::input_event;

#[input_event(channel_size = 2)]
#[derive(Clone, Copy, Debug)]
pub struct BatteryAdcEvent(pub u16);

#[input_event(channel_size = 2)]
#[derive(Clone, Copy, Debug)]
pub struct ChargingStateEvent {
    pub charging: bool,
}
```

**Parameters:**
- `channel_size` (optional): Buffer size of the event channel. Default is 8.

The `#[input_event]` macro generates a static channel and implements the `InputEvent` trait, enabling the event to be published and subscribed to.

::: tip Dual-channel events
`#[input_event]` can be combined with `#[controller_event]` on the same struct/enum to create a dual-channel event type. The macro order does not matter.

```rust
use rmk_macro::{controller_event, input_event};

#[input_event(channel_size = 4)]
#[controller_event(channel_size = 1, subs = 2)]
#[derive(Clone, Copy, Debug)]
pub struct SensorEvent {
    pub value: u16,
}
```
:::

::: note
Input event channels are single-consumer (`Channel`), meaning each input event type can only have **one** `InputProcessor` subscribing to it. This is different from controller events which use multi-subscriber `PubSubChannel`. If you need multiple consumers for the same event, consider using controller events instead.
:::

### Multi-event enums

When an input device produces multiple types of events, use `#[derive(InputEvent)]` on an enum to create a wrapper event type:

```rust
use rmk_macro::InputEvent;

#[derive(InputEvent, Clone, Debug)]
pub enum NrfAdcEvent {
    Pointing(PointingEvent),
    Battery(BatteryAdcEvent),
}
```

When a multi-event enum is published, each variant is automatically routed to its underlying concrete event channel. This means processors subscribe to the individual event types (e.g., `PointingEvent`, `BatteryAdcEvent`), not the wrapper enum.

### Built-in input events

RMK provides several built-in input event types:

- `KeyboardEvent` — Key press/release events from the matrix or encoders
- `PointingEvent` — Pointing device axis events (mouse movement, scroll)
- `BatteryAdcEvent` — Raw battery ADC readings
- `ChargingStateEvent` — Charging state changes
- `TouchpadEvent` — Touchpad input events

## InputDevice trait

The `InputDevice` trait defines the interface for input devices in RMK:

```rust
pub trait InputDevice: Runnable {
    type Event;
    async fn read_event(&mut self) -> Self::Event;
}
```

You don't need to implement this trait manually. Use the `#[input_device]` macro to generate the implementation automatically.

### Defining an input device

Use the `#[input_device]` macro on a struct to define an input device:

```rust
use rmk_macro::input_device;

#[input_device(publish = BatteryAdcEvent)]
pub struct MyBatteryReader {
    pin: u8,
}
```

**Parameters:**
- `publish = EventType` (required): The event type this device publishes.

**How it works:**
- `#[input_device]` implements both `InputDevice` and `Runnable` traits automatically.
- You only need to implement a `read_<event_name>_event()` method that **returns** the event. The macro will automatically publish the returned event to the corresponding event channel — you don't need to call any publish function yourself.
- The method name is derived from the event type name by converting it to snake_case and stripping the `Event` suffix. For example:
  - `publish = BatteryAdcEvent` → `async fn read_battery_adc_event(&mut self) -> BatteryAdcEvent`
  - `publish = KeyboardEvent` → `async fn read_keyboard_event(&mut self) -> KeyboardEvent`
  - `publish = NrfAdcEvent` → `async fn read_nrf_adc_event(&mut self) -> NrfAdcEvent`

### Single-event device example

A device that reads charging state from a GPIO pin:

```rust
use rmk_macro::input_device;

#[input_device(publish = ChargingStateEvent)]
pub struct ChargingStateReader<I: InputPin> {
    state_input: I,
    low_active: bool,
    current_charging_state: bool,
}

impl<I: InputPin> ChargingStateReader<I> {
    pub fn new(state_input: I, low_active: bool) -> Self {
        Self {
            state_input,
            low_active,
            current_charging_state: false,
        }
    }

    // This method is required by #[input_device(publish = ChargingStateEvent)]
    async fn read_charging_state_event(&mut self) -> ChargingStateEvent {
        loop {
            embassy_time::Timer::after_secs(5).await;
            let charging = if self.low_active {
                self.state_input.is_low().unwrap_or(false)
            } else {
                self.state_input.is_high().unwrap_or(false)
            };
            if charging != self.current_charging_state {
                self.current_charging_state = charging;
                return ChargingStateEvent { charging };
            }
        }
    }
}
```

### Multi-event device example

A device that produces multiple event types using a wrapper enum:

```rust
use rmk_macro::{InputEvent, input_device};

// Define a wrapper enum for multiple event types
#[derive(InputEvent, Clone, Debug)]
pub enum NrfAdcEvent {
    Pointing(PointingEvent),
    Battery(BatteryAdcEvent),
}

#[input_device(publish = NrfAdcEvent)]
pub struct NrfAdc<'a, const PIN_NUM: usize, const EVENT_NUM: usize> {
    saadc: Saadc<'a, PIN_NUM>,
    // ... other fields
}

impl<'a, const PIN_NUM: usize, const EVENT_NUM: usize> NrfAdc<'a, PIN_NUM, EVENT_NUM> {
    // Returns the wrapper enum
    async fn read_nrf_adc_event(&mut self) -> NrfAdcEvent {
        // Read ADC and return the appropriate variant
        NrfAdcEvent::Battery(BatteryAdcEvent(adc_value))
    }
}
```

## InputProcessor trait

The `InputProcessor` trait defines the interface for components that consume and process input events:

```rust
pub trait InputProcessor: Runnable {
    type Event: SubscribableInputEvent;
    async fn process(&mut self, event: Self::Event);
}
```

### Defining an input processor

Use the `#[input_processor]` macro to define a processor that subscribes to input events:

```rust
use rmk_macro::input_processor;

#[input_processor(subscribe = [BatteryAdcEvent, ChargingStateEvent])]
pub struct BatteryProcessor {
    battery_level: u8,
    charging: bool,
}

impl BatteryProcessor {
    pub fn new() -> Self {
        Self { battery_level: 0, charging: false }
    }

    // Handler for BatteryAdcEvent
    async fn on_battery_adc_event(&mut self, event: BatteryAdcEvent) {
        self.battery_level = convert_adc_to_percent(event.0);
    }

    // Handler for ChargingStateEvent
    async fn on_charging_state_event(&mut self, event: ChargingStateEvent) {
        self.charging = event.charging;
    }
}
```

**Parameters:**
- `subscribe = [Event1, Event2, ...]` (required): Event types to subscribe to.

**How it works:**
- `#[input_processor]` implements both `InputProcessor` and `Runnable` traits automatically. The macro will automatically subscribe to the specified event types and route incoming events to the corresponding handler methods.
- You only need to implement `on_<event_name>_event()` handler methods for each subscribed event type. The method name follows the same snake_case conversion as input devices. For example:
  - `subscribe = [BatteryAdcEvent]` → `async fn on_battery_adc_event(&mut self, event: BatteryAdcEvent)`
  - `subscribe = [ChargingStateEvent]` → `async fn on_charging_state_event(&mut self, event: ChargingStateEvent)`

::: warning
A struct can only be either an input device (`#[input_device]`) or an input processor (`#[input_processor]`), not both.
:::

## Running input devices and processors

All input devices and processors implement the `Runnable` trait. Use the `run_all!` macro to run multiple runnables concurrently:

```rust
use rmk::run_all;

// Create your devices and processors
let mut matrix = Matrix::new(row_pins, col_pins, debouncer);
let mut encoder = RotaryEncoder::new(pin_a, pin_b, 0);
let mut adc_device = NrfAdc::new(saadc, event_types, interval, None);
let mut batt_proc = BatteryProcessor::new(2000, 2806);

// Run them concurrently using join and run_all!
join(
    run_all!(matrix, encoder, adc_device, batt_proc),
    run_rmk(&keymap, driver, &stack, &mut storage, rmk_config),
).await;
```

## Configuration

RMK provides `keyboard.toml` configuration support for some built-in input devices (such as rotary encoders, joysticks, and PMW3610 sensors), so you can use them without writing any Rust code. See the [Input Device Configuration](../configuration/input_device) documentation for details.

## Advanced: Combining with `#[controller]`

Both `#[input_device]` and `#[input_processor]` can be combined with `#[controller]` on the same struct. This allows a single struct to both produce/process input events and subscribe to controller events. The macros will generate a unified `run` method that combines the logic of both — reading/processing input events and handling controller events concurrently.

```rust
use rmk_macro::{input_device, controller};

#[input_device(publish = SensorEvent)]
#[controller(subscribe = [ConfigEvent])]
pub struct InputSensor {
    pub threshold: u16,
}

impl InputSensor {
    // Required by #[input_device]: return the event, it will be published automatically
    async fn read_sensor_event(&mut self) -> SensorEvent {
        // Read sensor data
    }

    // Required by #[controller]: handle subscribed controller events
    async fn on_config_event(&mut self, event: ConfigEvent) {
        self.threshold = event.threshold;
    }
}
```

See the [Controller](./controller) documentation for more details on the `#[controller]` macro.
