# Input Device

RMK's input device system is a unified, event-driven framework designed to support diverse input devices.

::: info What are input devices?
Input devices are hardware components that provide input to the keyboard firmware, such as key matrix, rotary encoders, joysticks, pointing devices, ADC sensors, and more.
:::

## Event-driven input system

RMK uses an event-driven architecture for input handling. Input devices produce events, and processors consume and process those events. Events flow through type-safe channels, decoupling producers from consumers.

The complete event chain in RMK is:

```
┌────────────┐
│input device│
└─────┬──────┘
      │ publish
      ▼
┌────────────┐
│   events   │
└─────┬──────┘
      │ subscribe
      ▼
┌───────────────┐
│   processor   │
└───────────────┘
```

**Key concepts:**

- **Event** — A type-safe message carrying data from an input device (e.g., `KeyboardEvent`, `PointingEvent`, `BatteryAdcEvent`). Defined with the `#[event]` macro.
- **Input Device** — A hardware driver that reads from a peripheral and publishes events. Defined with the `#[input_device]` macro.
- **Processor** — A component that subscribes to events and processes them (e.g., converting raw ADC values to battery percentage). Defined with the `#[processor]` macro.
- **Runnable** — All input devices and processors implement the `Runnable` trait. Since this implementation is macro-generated, these components can be easily run concurrently using the `run_all!` macro.

## Event

Events are the messages that flow from input devices to processors. Each event type has its own dedicated channel, where it is published by an `InputDevice` and subscribed to by a `Processor`.

### Defining events

Use the `#[event]` macro to define a custom event:

```rust
use rmk_macro::event;

// MPSC channel (single consumer) - default
#[event(channel_size = 2)]
#[derive(Clone, Copy, Debug)]
pub struct BatteryAdcEvent(pub u16);

// PubSub channel (multiple subscribers)
#[event(channel_size = 2, subs = 4, pubs = 1)]
#[derive(Clone, Copy, Debug)]
pub struct ChargingStateEvent {
    pub charging: bool,
}
```

**Parameters:**
- `channel_size` (optional): Buffer size of the event channel. Default is 8 for MPSC, 1 for PubSub.
- `subs` (optional): Max subscribers. If specified, uses PubSub channel. Default is 4.
- `pubs` (optional): Max publishers. If specified, uses PubSub channel. Default is 1.

The `#[event]` macro generates a static channel and implements the `PublishableEvent`, `SubscribableEvent`, and `AsyncPublishableEvent` traits.

::: note Channel types
- **MPSC (default)**: Single-consumer channel. Use when only one processor subscribes to the event.
- **PubSub**: Multi-subscriber channel. Use when multiple processors need to receive the same event. Specify `subs` or `pubs` to enable.
:::

### Multi-event enums

When an input device produces multiple types of events, use `#[derive(Event)]` on an enum to create a wrapper event type:

```rust
use rmk_macro::Event;

#[derive(Event, Clone, Debug)]
pub enum NrfAdcEvent {
    Pointing(PointingEvent),
    Battery(BatteryAdcEvent),
}
```

When a multi-event enum is published, each variant is automatically routed to its underlying concrete event channel. This means processors subscribe to the individual event types (e.g., `PointingEvent`, `BatteryAdcEvent`), not the wrapper enum.

### Built-in events

RMK provides several built-in event types:

- `KeyboardEvent` — Key press/release events from the matrix or encoders
- `PointingEvent` — Pointing device axis events (mouse movement, scroll)
- `BatteryEvent` — Battery level events
- `LedIndicatorEvent` — LED indicator state changes
- `LayerChangeEvent` — Active layer changes

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
use rmk_macro::{Event, input_device};

// Define a wrapper enum for multiple event types
#[derive(Event, Clone, Debug)]
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

## Processor trait

The `Processor` trait defines the interface for components that consume and process events:

```rust
pub trait Processor: Runnable {
    type Event;
    fn subscriber() -> impl EventSubscriber<Event = Self::Event>;
    async fn process(&mut self, event: Self::Event);
}
```

### Defining a processor

Use the `#[processor]` macro to define a processor that subscribes to events:

```rust
use rmk_macro::processor;

#[processor(subscribe = [BatteryAdcEvent, ChargingStateEvent])]
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
- `poll_interval = N` (optional): Polling interval in milliseconds. When set, the processor will also call a `poll()` method periodically.

**How it works:**
- `#[processor]` implements `Processor` and `Runnable` traits automatically. The macro will automatically subscribe to the specified event types and route incoming events to the corresponding handler methods.
- You only need to implement `on_<event_name>_event()` handler methods for each subscribed event type. The method name follows the same snake_case conversion as input devices. For example:
  - `subscribe = [BatteryAdcEvent]` → `async fn on_battery_adc_event(&mut self, event: BatteryAdcEvent)`
  - `subscribe = [ChargingStateEvent]` → `async fn on_charging_state_event(&mut self, event: ChargingStateEvent)`

### Polling processor example

A processor that both handles events and performs periodic updates:

```rust
use rmk_macro::processor;

#[processor(subscribe = [LedIndicatorEvent], poll_interval = 500)]
pub struct BlinkingLedProcessor {
    led_on: bool,
    blink_enabled: bool,
}

impl BlinkingLedProcessor {
    // Handler for LedIndicatorEvent
    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        self.blink_enabled = event.caps_lock;
    }

    // Called every 500ms
    async fn poll(&mut self) {
        if self.blink_enabled {
            self.led_on = !self.led_on;
            // Toggle LED
        }
    }
}
```

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

## Advanced: Combining `#[input_device]` with `#[processor]`

`#[input_device]` can be combined with `#[processor]` on the same struct. This allows a single struct to both produce events and subscribe to other events. The macros will generate a unified `run` method that combines the logic of both — reading input events and handling subscribed events concurrently.

```rust
use rmk_macro::{input_device, processor};

#[input_device(publish = SensorEvent)]
#[processor(subscribe = [ConfigEvent])]
pub struct InputSensor {
    pub threshold: u16,
}

impl InputSensor {
    // Required by #[input_device]: return the event, it will be published automatically
    async fn read_sensor_event(&mut self) -> SensorEvent {
        // Read sensor data
    }

    // Required by #[processor]: handle subscribed events
    async fn on_config_event(&mut self, event: ConfigEvent) {
        self.threshold = event.threshold;
    }
}
```
