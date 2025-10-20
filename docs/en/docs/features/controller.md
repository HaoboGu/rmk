# Controller Support

RMK's controller system provides a unified interface for managing output devices like displays, LEDs, and other peripherals that respond to keyboard events. Controllers are software modules that implement the `Controller` trait and receive events from the keyboard through RMK's event system.

## Overview

### What are Controllers?

Controllers in RMK are software modules that manage external hardware components and respond to keyboard events. They provide:

- **Event-driven architecture** that reacts to keyboard state changes
- **Two execution modes**: event-driven and polling-based
- **Built-in TOML configuration** for LED indicators (NumLock, CapsLock, ScrollLock)
- **Custom controller support** through the `#[controller]` attribute

### Built-in Controllers

RMK includes several built-in controllers:

**LED Indicator Controllers:**
- NumLock, CapsLock, ScrollLock LED indicators
- Automatic configuration through keyboard.toml
- Support for active-high and active-low pins

**Battery LED Controller:**
- Battery level indication with different states
- Charging state visualization
- Configurable blinking patterns

## Controller Architecture

### Controller Trait

All controllers must implement the `Controller` trait:

```rust
pub trait Controller {
    /// Type of the received events
    type Event;

    /// Process the received event
    async fn process_event(&mut self, event: Self::Event);

    /// Block waiting for next message
    async fn next_message(&mut self) -> Self::Event;
}
```

### Execution Modes

Controllers can operate in two modes:

**Event-Driven Controllers:**
Controllers that react only to events implement `EventController` (auto-implemented for all `Controller`s):

```rust
impl Controller for MyEventController {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        match event {
            ControllerEvent::KeyboardIndicator(state) => {
                // Handle LED indicator changes
            },
            ControllerEvent::Battery(level) => {
                // Handle battery level changes  
            },
            _ => {}
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}
```

**Polling Controllers:**
Controllers that need periodic updates implement `PollingController`:

```rust
impl PollingController for MyPollingController {
    const INTERVAL: embassy_time::Duration = embassy_time::Duration::from_millis(100);

    async fn update(&mut self) {
        // Periodic update logic (e.g., LED animations, sensor readings)
    }
}
```

## Using Controllers

### TOML Configuration (Built-in Controllers)

Built-in LED indicators can be configured in keyboard.toml:

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

### Custom Controllers

Custom controllers are declared using the `#[controller(event)]` or `#[controller(poll)]` attribute within your keyboard module.
If `#[controller(event)]` is used the controller must implement `EventController` (or just `Controller`) and the `EventController::event_loop` method will be called.
If `#[controller(poll)]` is used the controller must implement `PollingController` and the `PollingController::polling_loop` method will be called.

A `p` variable containing the chip peripherals is in scope inside the function.
It's also possible to define extra interrupts using the `bind_interrupts!` macro.

```rust
#[rmk_keyboard]
mod keyboard {
    // ... keyboard configuration ...

    #[controller(event)]
    fn my_custom_controller() -> MyCustomController {
        // Initialize your controller
        let pin = Output::new(p.PIN_4, Level::Low, OutputDrive::Standard);
        MyCustomController::new(pin)
    }
}
```

## Controller Events

Controllers receive events from RMK through the `ControllerEvent` enum, which includes:

### Available Events

```rust
pub enum ControllerEvent {
    /// Key event with the associated action
    Key(KeyboardEvent, KeyAction),
    /// Battery percentage (0-100)
    Battery(u8),
    /// Charging state (true = charging, false = not charging)  
    ChargingState(bool),
    /// Active layer changed
    Layer(u8),
    /// Modifier combination changed
    Modifier(ModifierCombination),
    /// Words per minute typing speed
    Wpm(u16),
    /// Connection type (USB = 0, BLE = 1)
    ConnectionType(u8),
    /// LED indicator states (NumLock, CapsLock, ScrollLock, etc.)
    KeyboardIndicator(LedIndicator),
    /// Sleep state changed
    Sleep(bool),
    // ... and more
}
```

### Event Subscription

Controllers automatically receive events through the CONTROLLER_CHANNEL:

```rust
use crate::channel::{CONTROLLER_CHANNEL, ControllerSub};

pub struct MyController {
    sub: ControllerSub,
    // ... other fields
}

impl MyController {
    pub fn new() -> Self {
        Self {
            sub: unwrap!(CONTROLLER_CHANNEL.subscriber()),
            // ... initialize other fields
        }
    }
}
```

## Creating Custom Controllers

### Basic Controller Implementation

Here's a complete example of a custom LED controller:

```rust
use embedded_hal::digital::StatefulOutputPin;
use crate::channel::{CONTROLLER_CHANNEL, ControllerSub};
use crate::controller::Controller;
use crate::event::ControllerEvent;

pub struct CustomLedController<P: StatefulOutputPin> {
    pin: P,
    sub: ControllerSub,
    state: bool,
}

impl<P: StatefulOutputPin> CustomLedController<P> {
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            sub: unwrap!(CONTROLLER_CHANNEL.subscriber()),
            state: false,
        }
    }
}

impl<P: StatefulOutputPin> Controller for CustomLedController<P> {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        match event {
            ControllerEvent::Layer(layer) => {
                // Toggle LED based on layer
                if layer > 0 && !self.state {
                    let _ = self.pin.set_high();
                    self.state = true;
                } else if layer == 0 && self.state {
                    let _ = self.pin.set_low();
                    self.state = false;
                }
            }
            _ => {}
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}
```

### Polling Controller Example

For controllers that need periodic updates (like animations):

```rust
use crate::controller::PollingController;

impl<P: StatefulOutputPin> PollingController for BlinkingController<P> {
    const INTERVAL: embassy_time::Duration = embassy_time::Duration::from_millis(500);

    async fn update(&mut self) {
        // Toggle LED every 500ms when active
        if self.active {
            self.state = !self.state;
            if self.state {
                let _ = self.pin.set_high();
            } else {
                let _ = self.pin.set_low();
            }
        }
    }
}
```

## Advanced Usage

### Battery State Controller

RMK includes a built-in `BatteryLedController` that demonstrates both event handling and polling:

```rust
// Events set the state
ControllerEvent::Battery(level) => {
    if level < 10 {
        self.state = BatteryState::Low;
    } else {
        self.state = BatteryState::Normal;
    }
}

// Polling updates the LED based on state
async fn update(&mut self) {
    match self.state {
        BatteryState::Low => self.pin.toggle(),      // Blink for low battery
        BatteryState::Normal => self.pin.deactivate(), // Off for normal
        BatteryState::Charging => self.pin.activate(),  // On when charging
    }
}
```

### Multiple Controllers

You can define multiple controllers in your keyboard module:

```rust
#[rmk_keyboard]
mod keyboard {
    #[controller(event)]
    fn status_led() -> StatusLedController {
        StatusLedController::new(p.PIN_1)
    }

    #[controller(event)]
    fn layer_indicator() -> LayerLedController {
        LayerLedController::new(p.PIN_2)
    }

    #[controller(poll)]
    fn battery_monitor() -> BatteryController {
        BatteryController::new(p.PIN_3)
    }
}
```

### Split Keyboard

For split keyboard controller usage, see the [Split Keyboard Controllers](./split_keyboard#controllers-in-split-keyboards) section.
