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

- An associated `EventType` that defines what kind of events this device generates
- A `run()` method containing the device's main logic
- A `send_event()` method to send events to processors

Example implementation:

```rust
struct MyEncoder;
impl InputDevice for MyEncoder {
    type EventType = Event;
    
    async fn run(&mut self) {
        // Read encoder and send events
        let event = Event::RotaryEncoder(RotaryEncoderEvent::Clockwise);
        self.send_event(event).await;
    }
    
    async fn send_event(&mut self, event: Self::EventType) {
        // Send event to processor
    }
}
```

## Input Processors

Input processors handle events from input devices and convert them into HID reports. A processor:

- Receives events from one or more input devices
- Processes those events into HID reports
- Sends the reports to USB/BLE

The `InputProcessor` trait defines this behavior with:

- Associated types for events it handles (`EventType`) and reports it generates (`ReportType`)
- A `process()` method to convert events to reports
- A `read_event()` method to receive events
- A `send_report()` method to send processed reports
- A default `run()` implementation that handles the event processing loop

### Built-in Event Types

RMK provides several built-in event types through the `Event` enum:

- `Key` - Standard keyboard key events
- `RotaryEncoder` - Rotary encoder rotation events
- `Touchpad` - Multi-touch touchpad events
- `Joystick` - Joystick axis events
- `AxisEventStream` - Stream of axis events for complex input devices

### Running Multiple Devices

RMK provides macros to run multiple input devices and processors concurrently:

```rust
// Run multiple input devices
let encoder = MyEncoder::new();
let touchpad = MyTouchpad::new();

// Run multiple processors
let encoder_proc = EncoderProcessor::new();
let touchpad_proc = TouchpadProcessor::new();

join(
    run_processors!(encoder_proc, touchpad_proc),
    run_devices!(encoder, touchpad)
).await;
```
