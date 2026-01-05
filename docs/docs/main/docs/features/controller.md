# Controller Support

RMK's controller system provides a unified interface for managing output devices like displays, LEDs, and other peripherals that respond to keyboard events. Controllers are software modules that implement the `Controller` trait and receive events from the keyboard through RMK's event system.

## Overview

Controllers in RMK are software modules that manage external hardware components and respond to keyboard events. They provide:

- **Event-driven architecture** that reacts to keyboard state changes
- **Two execution modes**: event-driven and polling-based
- **Custom controller support** through the `#[controller]` attribute

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
    fn interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_millis(100)
    }

    async fn update(&mut self) {
        // Periodic update logic (e.g., LED animations, sensor readings)
    }
}
```

The polling interval can also be passed as a parameter like this:

```rust
struct ConfigurableController {
    interval: embassy_time::Duration,
}

impl ConfigurableController {
    /// Create a controller with a specific update frequency in Hz
    pub fn with_hz(hz: u32) -> Self {
        Self {
            interval: embassy_time::Duration::from_hz(hz as u64),
        }
    }
}

impl PollingController for ConfigurableController {
    fn interval(&self) -> embassy_time::Duration {
        self.interval
    }

    async fn update(&mut self) {
        // update periodic
    }
}

// Usage: create a controller with 60Hz update rate
let controller = ConfigurableController::with_hz(60);
```

### Controller Events

Controller events are signals sent from other parts of RMK and handled by `Controller`s. Each controller receives the [`ControllerEvent`](https://docs.rs/rmk/latest/rmk/event/enum.ControllerEvent.html) from the `CONTROLLER_CHANNEL` and reacts only to the events that it is responsible for.

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
If `#[controller(event)]` is used, the controller must implement `EventController` (or just `Controller`) and the `EventController::event_loop` method will be called.
If `#[controller(poll)]` is used, the controller must implement `PollingController` and the `PollingController::polling_loop` method will be called.

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

## Creating Custom Controllers

### Implement `Controller` Trait

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

impl<P: StatefulOutputPin> Controller for BlinkingController<P> {
    type Event = ControllerEvent;
    async fn process_event(&mut self, event: Self::Event) {
        match event {
            ControllerEvent::Layer(layer) => {
                // Set active when current layer is not 0
                if layer != 0 {
                    self.active = false;
                    self.pin.set_low();
                } else {
                    self.active = true;
                }
            }
            _ => {}
        }
    }
    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}

impl<P: StatefulOutputPin> PollingController for BlinkingController<P> {
    fn interval(&self) -> embassy_time::Duration {
        embassy_time::Duration::from_millis(500)
    }

    async fn update(&mut self) {
        // Toggle LED every 500ms when active (i.e., current layer is not 0)
        if self.active {
            self.pin.toggle();
        }
    }
}
```

## Controllers in Split Keyboards

Controllers can be used in split keyboards. Peripheral devices can use controllers to respond to events from the central, such as LED indicators for CapsLock state or layer changes.

### Peripheral Controllers

Peripheral devices can use controllers to manage local output devices like keyboard indicators. Events from the central (such as CapsLock state) are automatically synchronized to peripherals through the split communication protocol.

#### Example: CapsLock LED on Peripheral

Here's an example of implementing a CapsLock LED indicator on a split peripheral:

```rust
pub struct CapsLockController {
    led: Output<'static>,
    sub: ControllerSub,
    caps_lock_on: bool,
}

impl Controller for CapsLockController {
    type Event = ControllerEvent;

    async fn process_event(&mut self, event: Self::Event) {
        match event {
            ControllerEvent::KeyboardIndicator(state) => {
                if state.caps_lock() != self.caps_lock_on {
                    self.caps_lock_on = state.caps_lock();
                    // Update LED state
                    self.led.set_state(self.caps_lock_on);
                }
            }
            _ => {}
        }
    }

    async fn next_message(&mut self) -> Self::Event {
        self.sub.next_message_pure().await
    }
}

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    #[controller(event)]
    fn capslock_led() -> CapsLockController {
        let led = Output::new(p.PIN_4, Level::Low, OutputDrive::Standard);

        CapsLockController {
            led,
            sub: unwrap!(CONTROLLER_CHANNEL.subscriber()),
            caps_lock_on: false,
        }
    }
}
```
