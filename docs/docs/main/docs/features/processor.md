# Processor

RMK's processor system provides a unified interface for components that consume events and react to them, such as displays, LEDs, and other output peripherals.

## Overview

Processors subscribe to events and react accordingly. Events are published by [Input Devices](./input_device) or other processors. For details about events, see the [Event](./event) documentation.

Processors can operate in two modes:
- **Event-driven** - React to events as they arrive
- **Polling** - Perform periodic updates at specified intervals (in addition to handling events)

## Defining Processors

Use the `#[processor]` macro to define custom processors:

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
- `subscribe = [Event1, Event2, ...]` (required): Event types to subscribe to (see [Built-in Events](./event#built-in-events))
- `poll_interval = <ms>` (optional): Enable polling with fixed interval, requires `poll()` method

**How it works:**
- `#[processor]` implements `Processor` and `Runnable` traits automatically
- Event handlers are automatically routed based on method naming: `on_<event_name>_event()`
- Method names follow snake_case conversion of event type names

## Registering Processors

Processors need to be registered in your `#[rmk_keyboard]` module using the `#[register_processor]` attribute:

```rust
#[rmk_keyboard]
mod my_keyboard {
    use super::*;

    #[register_processor(event)]  // Event-driven mode
    fn my_processor() -> MyProcessor {
        MyProcessor::new()
    }
}
```

Available registration modes:
- `#[register_processor(event)]`: Event-driven mode, reacts to subscribed events
- `#[register_processor(poll)]`: Polling mode, requires `poll_interval` parameter in `#[processor]` macro

## Multi-event Subscription

Processors can subscribe to multiple event types and handle them with separate methods:

```rust
use rmk_macro::processor;

#[processor(subscribe = [LayerChangeEvent, BatteryStateEvent])]
pub struct MultiEventProcessor {
    layer: u8,
}

impl MultiEventProcessor {
    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.layer = event.layer;
        // Update display with new layer
    }

    async fn on_battery_state_event(&mut self, event: BatteryStateEvent) {
        // Update battery indicator
    }
}
```

## Polling Processor

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
use embedded_hal::digital::StatefulOutputPin;

#[processor(subscribe = [LedIndicatorEvent])]
pub struct CapsLockLed<P: StatefulOutputPin> {
    led: P,
    low_active: bool,
}

impl<P: StatefulOutputPin> CapsLockLed<P> {
    pub fn new(pin: P, low_active: bool) -> Self {
        Self {
            led: pin,
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

## Related Documentation

- [Event](./event) - Event concepts, built-in events, and custom event definition
- [Input Device](./input_device) - How to create input devices that publish events
