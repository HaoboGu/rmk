# Processor (Controller Support)

RMK's processor system provides a unified interface for managing output devices like displays, LEDs, and other peripherals that respond to keyboard events.

::: tip Unified API
The `#[processor]` macro replaces the previous `#[controller]` and `#[input_processor]` macros, providing a unified way to define event-driven components.
:::

## Overview

RMK uses an event-driven architecture where event producers (keyboard, BLE stack, etc.) are decoupled from event consumers (processors). This allows processors to independently react to specific events they care about.

The complete event chain can be found in the [Input Device](./input_device#event-driven-input-system) documentation.

**Key concepts:**
- **Events** - Carry state changes and keyboard events through type-safe channels
- **Processors** - Subscribe to events and react accordingly

Processors can operate in two modes:
- **Event-driven** - React to events as they arrive
- **Polling** - Subscribe to events and perform periodic updates at specified intervals

## Built-in Features

RMK provides built-in events and processors that you can use directly without writing custom code.

### Built-in Events

RMK provides a type-safe event system where each event type has its own dedicated channel. The following built-in event types are available for processors to subscribe:

**Keyboard State Events:**
- `LayerChangeEvent` - Active layer changed
- `LedIndicatorEvent` - LED indicator state changed (NumLock, CapsLock, ScrollLock)
- `WpmUpdateEvent` - Words per minute updated
- `SleepStateEvent` - Sleep state changed

**Input Events:**
- `KeyboardEvent` - Key press/release event
- `PointingEvent` - Pointing device events

**Connection Events:**
- `ConnectionChangeEvent` - Connection type changed (USB/BLE)

**BLE Events** (when BLE is enabled):
- `BleStateChangeEvent` - BLE connection state changed
- `BleProfileChangeEvent` - BLE profile switched

**Power Events** (when BLE is enabled):
- `BatteryEvent` - Battery state changed (includes level and charging status)

**Split Keyboard Events** (when split is enabled):
- `PeripheralConnectedEvent` - Peripheral connection state changed
- `CentralConnectedEvent` - Connected to central state changed
- `PeripheralBatteryEvent` - Peripheral battery state changed

### Built-in LED Indicator

RMK provides built-in LED indicator support for NumLock, CapsLock, and ScrollLock. These can be easily configured in `keyboard.toml` without writing any code:

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

## Custom Processors

RMK's processor system is **designed for easy extension without modifying core code**. You can define custom processors using the `#[processor]` macro to extend keyboard functionality for displays, sensors, LEDs, and any other peripherals.

### Defining Processors

Processors are defined using the `#[processor]` attribute macro on structs:

```rust
use rmk_macro::processor;

#[processor(subscribe = [LayerChangeEvent])]
pub struct MyProcessor {
    // Your processor fields
}

impl MyProcessor {
    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        // Handle layer changes
    }
}
```

**Parameters:**
- `subscribe = [Event1, Event2, ...]` (required): Event types to subscribe to
- `poll_interval = <ms>` (optional): Enable polling with fixed interval, requires `poll()` method

**How it works:**
- `#[processor]` implements `Processor` and `Runnable` traits automatically
- Event handlers are automatically routed based on method naming: `on_<event_name>_event()`
- Method names follow snake_case conversion of event type names

### Registering Processors

Processors need to be registered in your `#[rmk_keyboard]` module using the `#[register_controller]` attribute:

```rust
#[rmk_keyboard]
mod my_keyboard {
    use super::*;

    #[register_controller(event)]  // Event-driven mode
    fn my_processor() -> MyProcessor {
        MyProcessor::new()
    }
}
```

Available registration modes:
- `#[register_controller(event)]`: Event-driven mode, reacts to subscribed events
- `#[register_controller(poll)]`: Polling mode, requires `poll_interval` parameter in `#[processor]` macro

### Multi-event Subscription

Processors can subscribe to multiple event types and handle them with separate methods:

```rust
use rmk_macro::processor;

#[processor(subscribe = [LayerChangeEvent, BatteryEvent])]
pub struct MultiEventProcessor {
    layer: u8,
    battery_level: u8,
}

impl MultiEventProcessor {
    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.layer = event.layer;
        // Update display with new layer
    }

    async fn on_battery_event(&mut self, event: BatteryEvent) {
        self.battery_level = event.level;
        // Update battery indicator
    }
}
```

### Polling Processor

For processors that need periodic updates (e.g., display refresh, LED animations), use the `poll_interval` parameter:

```rust
use rmk_macro::processor;

#[processor(subscribe = [LayerChangeEvent], poll_interval = 500)]
pub struct DisplayProcessor<D: DrawTarget> {
    display: D,
    layer: u8,
    needs_refresh: bool,
}

impl<D: DrawTarget> DisplayProcessor<D> {
    pub fn new(display: D) -> Self {
        Self {
            display,
            layer: 0,
            needs_refresh: true,
        }
    }

    // Event handler - triggered when layer changes
    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.layer = event.layer;
        self.needs_refresh = true;
    }

    // Called every 500ms
    async fn poll(&mut self) {
        if self.needs_refresh {
            self.render_layer();
            self.needs_refresh = false;
        }
    }

    fn render_layer(&mut self) {
        // Render current layer to display
    }
}
```

## Example: LED Indicator Processor

A complete example of a processor that controls an LED based on keyboard indicators:

```rust
use rmk_macro::processor;
use embassy_hal::gpio::{Output, Level};

#[processor(subscribe = [LedIndicatorEvent])]
pub struct CapsLockLed<'a> {
    led: Output<'a>,
    low_active: bool,
}

impl<'a> CapsLockLed<'a> {
    pub fn new(pin: impl Peripheral<P = impl Pin>, low_active: bool) -> Self {
        let initial = if low_active { Level::High } else { Level::Low };
        Self {
            led: Output::new(pin, initial, Speed::Low),
            low_active,
        }
    }

    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        let should_light = event.indicator.caps_lock;
        if self.low_active {
            if should_light { self.led.set_low() } else { self.led.set_high() }
        } else {
            if should_light { self.led.set_high() } else { self.led.set_low() }
        }
    }
}
```

## Custom Events

In addition to built-in events, you can define custom event types using the `#[event]` macro. Custom events work seamlessly alongside built-in events and follow the same usage patterns.

### Defining Custom Events

Use the `#[event]` macro to define custom events:

```rust
use rmk_macro::event;

#[event(channel_size = 8, subs = 2, pubs = 1)]
#[derive(Clone, Copy, Debug)]
pub struct DisplayUpdateEvent {
    pub content: DisplayContent,
}
```

**Parameters:**
- `channel_size`: Buffer size for the event channel
- `subs`: Maximum number of subscribers (triggers PubSub channel mode)
- `pubs`: Maximum number of publishers (triggers PubSub channel mode)

### Publishing Events

Events are published using the `publish_event` or `publish_event_async` functions:

```rust
use rmk::event::{publish_event, publish_event_async};

// Synchronous (immediate, non-blocking)
publish_event(DisplayUpdateEvent { content: DisplayContent::Layer(1) });

// Asynchronous (awaitable, may block if channel is full)
publish_event_async(DisplayUpdateEvent { content: DisplayContent::Layer(1) }).await;
```

### Subscribing to Custom Events

Processors can subscribe to custom events the same way as built-in events:

```rust
use rmk_macro::processor;

#[processor(subscribe = [DisplayUpdateEvent])]
pub struct DisplayController {
    // ...
}

impl DisplayController {
    async fn on_display_update_event(&mut self, event: DisplayUpdateEvent) {
        // Handle the custom event
    }
}
```
