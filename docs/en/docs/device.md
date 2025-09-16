# Input Device Framework

RMK's input device framework provides a flexible and extensible way to handle various input sources beyond traditional key matrices. This document explains how the framework works and how to implement custom input devices.

## Overview

The input device system enables RMK to work with diverse input sources through a unified trait-based architecture:

- **InputDevice trait** - For hardware that generates input events
- **InputProcessor trait** - For processing and converting events to HID reports
- **Event system** - Type-safe event routing through channels
- **Macro support** - Convenient macros for running devices concurrently

## Framework Architecture

The framework follows this data flow:

1. **Input Devices** read physical hardware and generate `Event`s
2. **Events** are sent through channels (KEY_EVENT_CHANNEL for keyboard events, EVENT_CHANNEL for others)
3. **Input Processors** receive events and convert them into HID reports
4. **HID Reports** are transmitted via USB/BLE through KEYBOARD_REPORT_CHANNEL

## Event System

### Event Types

Events are defined in `rmk/src/event.rs` with the `Event` enum:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Event {
    /// Standard keyboard events (keys, rotary encoders)
    Key(KeyboardEvent),
    /// Multi-touch touchpad events
    Touchpad(TouchpadEvent),
    /// Joystick events with x,y,z axes
    Joystick([AxisEvent; 3]),
    /// Stream of axis events for variable-axis devices
    AxisEventStream(AxisEvent),
    /// Battery percentage event
    Battery(u16),
    /// Charging state changed event
    ChargingState(bool),
    /// End-of-stream marker for AxisEventStream
    Eos,
    /// Custom event with 16-byte payload
    Custom([u8; 16]),
}
```

### Event Routing

- **KeyboardEvent**: Automatically routed to KEY_EVENT_CHANNEL and processed by the built-in Keyboard processor
- **Other Events**: Sent to EVENT_CHANNEL and require custom processors for handling

## Core Traits

### InputDevice Trait

All input devices must implement this trait:

```rust
pub trait InputDevice {
    /// Read raw input events from the device
    async fn read_event(&mut self) -> Event;
}
```

**Example Implementation (Rotary Encoder):**

```rust
impl<A, B, P> InputDevice for RotaryEncoder<A, B, P>
where
    A: InputPin + Wait,  // When async_matrix feature is enabled
    B: InputPin + Wait,
    P: Phase,
{
    async fn read_event(&mut self) -> Event {
        // Handle pending release event first
        if let Some(last_action) = self.last_action {
            embassy_time::Timer::after_millis(5).await;
            let e = Event::Key(KeyboardEvent::rotary_encoder(self.id, last_action, false));
            self.last_action = None;
            return e;
        }

        loop {
            // Wait for pin changes asynchronously
            #[cfg(feature = "async_matrix")]
            {
                let (pin_a, pin_b) = self.pins();
                embassy_futures::select::select(
                    pin_a.wait_for_any_edge(), 
                    pin_b.wait_for_any_edge()
                ).await;
            }

            let direction = self.update();
            if direction != Direction::None {
                self.last_action = Some(direction);
                return Event::Key(KeyboardEvent::rotary_encoder(self.id, direction, true));
            }

            // Prevent busy-loop when not using async
            #[cfg(not(feature = "async_matrix"))]
            embassy_time::Timer::after_millis(20).await;
        }
    }
}
```

### InputProcessor Trait

Custom processors handle non-keyboard events:

```rust
pub trait InputProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    /// Process incoming events and return processing result
    async fn process(&mut self, event: Event) -> ProcessResult;
    
    /// Send processed reports (default implementation provided)
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }
    
    /// Get the current keymap
    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>;
}
```

**ProcessResult enum:**
```rust
pub enum ProcessResult {
    /// Continue processing the event with the next processor
    Continue(Event),
    /// Stop processing this event
    Stop,
}
```

### Runnable Trait

For components that run continuously:

```rust
pub trait Runnable {
    async fn run(&mut self);
}
```

The built-in `Keyboard` processor implements this trait and handles all KeyboardEvent processing.

## System Integration

### Basic Setup (Keyboard Events Only)

For keyboards that only generate keyboard events (most common case):

```rust
use rmk::{run_devices, run_rmk};
use rmk::channel::EVENT_CHANNEL;
use rmk::futures::future::join3;

join3(
    // Run input devices
    run_devices! (
        (matrix) => EVENT_CHANNEL,
    ),
    // Run keyboard processor
    keyboard.run(),
    // Run RMK system
    run_rmk(&keymap, driver, &mut storage, rmk_config),
).await;
```

### Advanced Setup (Custom Processors)

For devices requiring custom processing:

```rust
use rmk::{run_devices, run_processor_chain};

join3(
    // Run all input devices
    run_devices! (
        (matrix, battery_reader) => EVENT_CHANNEL,
    ),
    // Process events through custom processor chain
    run_processor_chain! {
        EVENT_CHANNEL => [battery_processor]
    },
    // Run keyboard and RMK system
    join(keyboard.run(), run_rmk(&keymap, driver, &mut storage, rmk_config)),
).await;
```

## Built-in Input Devices

RMK includes several built-in input devices:

### Matrix
The standard keyboard matrix scanner that generates KeyboardEvent for key presses and releases.

### Rotary Encoder
RMK provides a comprehensive rotary encoder implementation with multiple phase detection algorithms:

```rust
use rmk::input_device::rotary_encoder::{RotaryEncoder, DefaultPhase};

// Basic encoder with default phase detection
let encoder = RotaryEncoder::new(pin_a, pin_b, encoder_id);

// Encoder with custom resolution and direction
let encoder = RotaryEncoder::with_resolution(pin_a, pin_b, resolution, reverse, encoder_id);
```

**Supported Phase Algorithms:**
- `DefaultPhase` - Standard quadrature decoding
- `E8H7Phase` - Optimized for E8H7 encoder type
- `ResolutionPhase` - Configurable resolution with pulse counting

### Battery Monitoring
Built-in battery monitoring through ADC and charging state detection:

```rust
use rmk::input_device::battery::{ChargingStateReader, BatteryProcessor};

// Charging state reader
let charging_reader = ChargingStateReader::new(charging_pin, low_active);

// Battery processor for ADC values
let battery_processor = BatteryProcessor::new(adc_measured, adc_total, &keymap);
```

## Creating Custom Input Devices

### Step 1: Define Your Device Struct

```rust
use rmk::input_device::InputDevice;
use rmk::event::Event;

pub struct CustomSensor {
    adc: MyAdc,
    threshold: u16,
    last_value: u16,
}

impl CustomSensor {
    pub fn new(adc: MyAdc, threshold: u16) -> Self {
        Self {
            adc,
            threshold,
            last_value: 0,
        }
    }
}
```

### Step 2: Implement InputDevice Trait

```rust
impl InputDevice for CustomSensor {
    async fn read_event(&mut self) -> Event {
        loop {
            let current_value = self.adc.read().await.unwrap_or(0);
            
            // Only generate events when value changes significantly
            if (current_value as i32 - self.last_value as i32).abs() > self.threshold as i32 {
                self.last_value = current_value;
                return Event::Custom([
                    current_value as u8,
                    (current_value >> 8) as u8,
                    // ... fill remaining bytes as needed
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ]);
            }
            
            // Wait before next reading
            embassy_time::Timer::after_millis(100).await;
        }
    }
}
```

## Creating Custom Processors

### Step 1: Define Your Processor

```rust
use rmk::input_device::{InputProcessor, ProcessResult};
use core::cell::RefCell;

pub struct CustomProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, 0>>,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> 
    CustomProcessor<'a, ROW, COL, NUM_LAYER> 
{
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, 0>>) -> Self {
        Self { keymap }
    }
}
```

### Step 2: Implement InputProcessor Trait

```rust
impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> 
    InputProcessor<'a, ROW, COL, NUM_LAYER> for CustomProcessor<'a, ROW, COL, NUM_LAYER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::Custom(data) => {
                // Process custom event
                let value = data[0] as u16 | ((data[1] as u16) << 8);
                
                // Convert to mouse movement or other HID report
                if value > 100 {
                    let mouse_report = MouseReport {
                        x: (value / 10) as i8,
                        y: 0,
                        buttons: 0,
                        wheel: 0,
                        pan: 0,
                    };
                    
                    self.send_report(Report::Mouse(mouse_report)).await;
                }
                
                // Stop processing this event
                ProcessResult::Stop
            }
            _ => {
                // Let other processors handle this event
                ProcessResult::Continue(event)
            }
        }
    }

    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, 0>> {
        self.keymap
    }
}
```

## Advanced Patterns

### Multiple Device Types

Run different types of input devices together:

```rust
run_devices! (
    (matrix, encoder1, encoder2) => EVENT_CHANNEL,
    (battery_reader, custom_sensor) => EVENT_CHANNEL,
)
```

### Processor Chains

Chain multiple processors for complex event handling:

```rust
run_processor_chain! {
    EVENT_CHANNEL => [sensor_processor, battery_processor, custom_processor]
}
```

Events are processed in order. If any processor returns `ProcessResult::Stop`, the chain stops for that event.

### Channel Routing

Use different channels for different event types:

```rust
use rmk::channel::{Channel, blocking_mutex::raw::NoopRawMutex};

// Create custom channel
static SENSOR_CHANNEL: Channel<NoopRawMutex, Event, 8> = Channel::new();

run_devices! (
    (matrix) => EVENT_CHANNEL,           // Standard keyboard events
    (sensors) => SENSOR_CHANNEL,         // Custom sensor events
)
```

## Key Concepts

### Event Channels
- **KEY_EVENT_CHANNEL**: Automatically used for KeyboardEvent by run_devices! macro
- **EVENT_CHANNEL**: Default channel for non-keyboard events
- **KEYBOARD_REPORT_CHANNEL**: Output channel for all HID reports

### Async Design
All input devices run as separate async tasks, enabling:
- Non-blocking I/O with hardware
- Concurrent processing of multiple input sources
- Efficient resource utilization

### Macro Magic
The `run_devices!` macro automatically:
- Routes KeyboardEvent to KEY_EVENT_CHANNEL
- Handles channel overflow by dropping oldest events
- Joins all device tasks concurrently