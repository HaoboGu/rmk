# Controller Support

RMK's controller system provides a unified interface for managing output devices like displays, LEDs, and other peripherals that respond to keyboard events.

## Overview

RMK uses an event-driven architecture where event producers (keyboard, BLE stack, etc.) are decoupled from event consumers (controllers). This allows controllers to independently react to specific events they care about.

The complete event chain can be found in the [Input Device](./input_device#event-driven-input-system) documentation. This page focuses on the bottom half of the chain (controller events → controller).

**Key concepts:**
- **Events** - Carry state changes and keyboard events through type-safe channels
- **Controllers** - Subscribe to events and react accordingly

Controllers can operate in two modes:
- **Event-driven** - React to controller events as they arrive
- **Polling** - Subscribe to controller events and perform periodic updates at specified intervals

## Built-in Features

RMK provides built-in events and controllers that you can use directly without writing custom code.

### Built-in Events

RMK provides a type-safe event system where each event type has its own dedicated channel. The following built-in event types are available for controllers to subscribe:

**Keyboard State Events:**
- `LayerChangeEvent` - Active layer changed
- `LedIndicatorEvent` - LED indicator state changed (NumLock, CapsLock, ScrollLock)
- `WpmUpdateEvent` - Words per minute updated
- `SleepStateEvent` - Sleep state changed

**Input Events:**
- `KeyEvent` - Key press/release event with processed key action
- `ModifierEvent` - Modifier keys combination changed

**Connection Events:**
- `ConnectionChangeEvent` - Connection type changed (USB/BLE)

**BLE Events** (when BLE is enabled):
- `BleStateChangeEvent` - BLE connection state changed
- `BleProfileChangeEvent` - BLE profile switched

**Power Events** (when BLE is enabled):
- `BatteryStateEvent` - Battery state changed (includes level and charging status)

**Split Keyboard Events** (when split is enabled):
- `PeripheralConnectedEvent` - Peripheral connection state changed
- `CentralConnectedEvent` - Connected to central state changed
- `PeripheralBatteryEvent` - Peripheral battery state changed (includes id and BatteryStateEvent)
- `ClearPeerEvent` - Clear BLE peer information (BLE split only)

### Built-in Controllers

RMK provides built-in LED indicator controllers for NumLock, CapsLock, and ScrollLock. These can be easily configured in `keyboard.toml` without writing any code:

```toml
[light]
# NumLock LED
numslock.pin = "PIN_1"
numslock.low_active = false

# CapsLock LED
capslock.pin = "PIN_2"
capslock.low_active = true

# ScrollLock LED
scrolllock.pin = "PIN_3"
scrolllock.low_active = false
```

The LED indicators automatically subscribe to `LedIndicatorEvent` and update based on host keyboard state.

## Custom Controllers

RMK's controller system is **designed for easy extension without modifying core code**. You can define custom controllers using the `#[controller]` macro to extend keyboard functionality for displays, sensors, LEDs, and any other peripherals.

### Defining Controllers

Controllers are defined using the `#[controller]` attribute macro on structs:

```rust
use rmk_macro::controller;

#[controller(subscribe = [LayerChangeEvent])]
pub struct MyController {
    // Your controller fields
}

impl MyController {
    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        // Handle layer changes
    }
}
```

**Parameters:**
- `subscribe = [Event1, Event2, ...]` (required): Event types to subscribe to
- `poll_interval = <ms>` (optional): Enable polling with fixed interval, requires `poll()` method

**How it works:**
- `#[controller]` implements `Controller` trait automatically
- Routes events to `on_<event_name>_event()` handler methods, where `<event_name>` is a snake case name converted from the subscribed event. For example, if your controller subscribes to `BatteryStateEvent`, then `async fn on_battery_state_event(&mut self, event: BatteryStateEvent)` should be implemented
- If `poll_interval` is set, the controller operates in **polling mode**, a `poll()` method is required. `poll()` will be called at every `poll_interval`

### Registering Controllers

Register controllers in your keyboard module with `#[register_controller]`:

```rust
#[rmk_keyboard]
mod keyboard {
    #[register_controller(event)]
    fn battery_led() -> BatteryLedController {
        let pin = Output::new(p.PIN_4, Level::Low, OutputDrive::Standard);
        BatteryLedController::new(pin, false)
    }
}
```

**Execution modes:**
- `#[register_controller(event)]`: Event-driven only, responds to events as they arrive
- `#[register_controller(poll)]`: Event-driven + periodic polling, requires `poll_interval` parameter in `#[controller]` macro

Inside the registration function:
- `p` variable provides access to chip peripherals
- Use `bind_interrupts!` macro if additional interrupts are needed

### Running Controllers with Rust API

When using the Rust API (without `#[register_controller]`), controllers implement the `Runnable` trait and can be run using the `run_all!` macro, just like input devices and processors:

```rust
use rmk::run_all;

let mut my_controller = MyController::new();
let mut another_controller = AnotherController::new();

// Run controllers concurrently with other runnables
join(
    run_all!(matrix, encoder, batt_proc),
    run_all!(my_controller, another_controller),
).await;
```

### Examples

#### Event-based Controller

Controllers can subscribe to one or multiple event types. This example monitors layer changes and battery state:

```rust
use rmk_macro::controller;
use rmk::event::{LayerChangeEvent, BatteryStateEvent};

// Subscribe to multiple events
#[controller(subscribe = [LayerChangeEvent, BatteryStateEvent])]
pub struct StatusController {
    current_layer: u8,
    battery_level: u8,
}

impl StatusController {
    pub fn new() -> Self {
        Self {
            current_layer: 0,
            battery_level: 100,
        }
    }

    // Handler for LayerChangeEvent
    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.current_layer = event.layer;
        info!("Layer: {}", event.layer);
    }

    // Handler for BatteryStateEvent
    async fn on_battery_state_event(&mut self, event: BatteryStateEvent) {
        match event {
            BatteryStateEvent::Normal(level) => {
                self.battery_level = level;
            },
            _ => return,
        };
        info!("Battery state: {:?}", event);
    }
}
```

Register with `#[register_controller(event)]`:

```rust
#[rmk_keyboard]
mod keyboard {
    #[register_controller(event)]
    fn status_controller() -> StatusController {
        StatusController::new()
    }
}
```

#### Polling Controller

Blinking LED when layer 0 is activated, using `poll_interval` parameter:

```rust
use rmk_macro::controller;
use rmk::event::LayerChangeEvent;
use embedded_hal::digital::StatefulOutputPin;

#[controller(subscribe = [LayerChangeEvent], poll_interval = 500)]
pub struct BlinkingController<P: StatefulOutputPin> {
    pin: P,
    active: bool,
}

impl<P: StatefulOutputPin> BlinkingController<P> {
    pub fn new(pin: P) -> Self {
        Self { pin, active: true }
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.active = event.layer == 0;
        if !self.active {
            let _ = self.pin.set_low();
        }
    }

    // Called every 500ms automatically
    async fn poll(&mut self) {
        if self.active {
            let _ = self.pin.toggle();
        }
    }
}
```

Register with `#[register_controller(poll)]`:

```rust
#[rmk_keyboard]
mod keyboard {
    #[register_controller(poll)]
    fn blinking_led() -> BlinkingController {
        let pin = Output::new(p.PIN_5, Level::Low, OutputDrive::Standard);
        BlinkingController::new(pin)
    }
}
```

#### Split Keyboard Controller

CapsLock LED on peripheral (events auto-sync from central):

```rust
use rmk_macro::controller;
use rmk::event::LedIndicatorEvent;
use embassy_nrf::gpio::Output;

#[controller(subscribe = [LedIndicatorEvent])]
pub struct CapsLockController {
    led: Output<'static>,
    caps_lock_on: bool,
}

impl CapsLockController {
    pub fn new(led: Output<'static>) -> Self {
        Self { led, caps_lock_on: false }
    }

    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        let new_state = event.indicator.caps_lock();
        if new_state != self.caps_lock_on {
            self.caps_lock_on = new_state;
            if new_state {
                self.led.set_high();
            } else {
                self.led.set_low();
            }
        }
    }
}

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    #[register_controller(event)]
    fn capslock_led() -> CapsLockController {
        let led = Output::new(p.PIN_4, Level::Low, OutputDrive::Standard);
        CapsLockController::new(led)
    }
}
```

## Custom Events

In addition to built-in controller events, you can define custom event types using the `#[controller_event]` macro. Custom events work seamlessly alongside built-in events and follow the same usage patterns.

### Defining Custom Events

Use the `#[controller_event]` macro to define custom events:

```rust
use rmk_macro::controller_event;

#[controller_event(channel_size = 8, subs = 2)]
#[derive(Clone, Copy, Debug)]
pub struct BacklightEvent {
    pub brightness: u8,
}
```

::: tip Dual-channel events
`#[controller_event]` can be combined with `#[input_event]` on the same struct/enum to create a dual-channel event type. The macro order does not matter.

```rust
use rmk_macro::{controller_event, input_event};

#[controller_event(channel_size = 1, subs = 2)]
#[input_event(channel_size = 4)]
#[derive(Clone, Copy, Debug)]
pub struct SensorEvent {
    pub value: u16,
}
```
:::

**Macro parameters:**
- `channel_size` (optional): Buffer size for `PubSubChannel`. Default is 1
- `subs` (optional): Maximum number of subscribers. Default is 4
- `pubs` (optional): Maximum number of async publishers. Default is 1

The `#[controller_event]` macro uses `PubSubChannel` for all events, which buffers events with configurable capacity and supports both immediate (non-blocking) and async (awaitable) publishing.

**Choosing buffer size:**
- Use `channel_size = 1`(which is the default value) for state-like events (layer, battery level, connection state) where only the latest value matters
- Use larger buffer sizes (e.g., `channel_size = 8`) for event streams that need history (key events, input events)

### Publishing Custom Events

Events can be published from anywhere in your code:

**Non-blocking publish:**

```rust
use rmk::event::publish_controller_event;

publish_controller_event(BacklightEvent { brightness: 50 });
```

This publishes immediately and drops the event if the buffer is full.

**Async publish:**

```rust
use rmk::event::publish_controller_event_async;

publish_controller_event_async(BacklightEvent { brightness: 75 }).await;
```

This waits if the buffer is full (backpressure).

::: warning
When using `publish_controller_event_async()`, ensure at least one subscriber exists to avoid infinite blocking.
:::

### Complete Example

Here's a complete example showing how to define a custom event, create a controller for it, and publish it:

```rust
use rmk_macro::{controller_event, controller};
use rmk::event::publish_controller_event;

// 1. Define custom event
#[controller_event(channel_size = 8, subs = 2)]
#[derive(Clone, Copy, Debug)]
pub struct DisplayUpdateEvent {
    pub line: u8,
    pub text: [u8; 16],
}

// 2. Create controller that subscribes to it
#[controller(subscribe = [DisplayUpdateEvent])]
pub struct DisplayController {
    // ... display hardware fields
}

impl DisplayController {
    pub fn new() -> Self {
        Self { /* ... */ }
    }

    async fn on_display_update_event(&mut self, event: DisplayUpdateEvent) {
        // Update display with event.line and event.text
    }
}

// 3. Register the controller
#[rmk_keyboard]
mod keyboard {
    #[register_controller(event)]
    fn display_controller() -> DisplayController {
        DisplayController::new()
    }
}

// 4. Publish from anywhere in your code
publish_controller_event(DisplayUpdateEvent {
    line: 0,
    text: *b"Hello RMK!      ",
});
```
