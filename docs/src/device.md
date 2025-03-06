# Device

There are two types of device in RMK:

- input device: an external device which generates HID reports (keyboard/mouse/media), such as encoder, joystick, touchpad, etc.

- output device: an external device which is triggered by RMK, to perform some functionalities, such as LED, RGB, screen, motor, etc

## Input device

Here is a simple(but not exhaustive) list of input devices:

- Keyboard itself
- Rotary encoder
- Touchpad
- Trackball
- Joystick

Except keyboard and rotary encoder, the others protocol/implementation depend on the actual device. A driver like interface is what we need.

### Rotary encoder

The encoder list is represented separately in vial, different from normal matrix. But layers still have effect on encoder. The behavior of rotary encoder could be changed by vial.

# Input Devices

RMK supports various input devices beyond just key matrices. The input system consists of two main components:

### Input Device Trait

Each input device must implement the `InputDevice` trait, which requires:

- A `read_event()` method to read raw input events

Example implementation:

```rust
struct MyEncoder;
impl InputDevice for MyEncoder {
    async fn read_event(&mut self) -> Event {
        // Read encoder and return events
        embassy_time::Timer::after_secs(1).await;
        Event::RotaryEncoder(RotaryEncoderEvent::Clockwise)
    }
}
```

### Runnable Trait

For components that need to continuously run in the background, RMK provides the `Runnable` trait:

```rust
pub trait Runnable {
    async fn run(&mut self);
}
```

The `Keyboard` type implements this trait to process events and generate reports.

## Input Processors

Input processors handle events from input devices and convert them into HID reports. A processor:

- Receives events from one or more input devices
- Processes those events into HID reports
- Sends the reports to USB/BLE

The `InputProcessor` trait defines this behavior with:

- A `process()` method to convert events to reports
- A `send_report()` method to send processed reports

### Built-in Event Types

RMK provides several built-in event types through the `Event` enum:

- `Key` - Standard keyboard key events
- `RotaryEncoder` - Rotary encoder rotation events
- `Touchpad` - Multi-touch touchpad events
- `Joystick` - Joystick axis events
- `AxisEventStream` - Stream of axis events for complex input devices

### Running Devices, Processors and RMK

RMK provides a standardized approach for running the entire system with multiple components:

```rust
// Start the system with three concurrent tasks
join3(
    // Task 1: Run all input devices and send events to EVENT_CHANNEL
    run_devices! (
        (matrix, encoder) => EVENT_CHANNEL,
    ),
    // Task 2: Run the keyboard processor
    keyboard.run(),
    // Task 3: Run the main RMK system
    run_rmk(&keymap, driver, storage, light_controller, rmk_config),
)
.await;
```

This pattern is used across all RMK examples and provides a clean way to:
1. Read events from input devices and send them to a channel
2. Process those events with a keyboard processor
3. Handle all RMK system functionality in parallel
