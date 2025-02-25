# Device

There are two types of device in RMK:

- input device: an external device which finally generates a HID(keyboard/mouse/media) report, such as encoder, joystick, touchpad, etc.

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

## Input Devices

Input devices are physical components that generate input events. These can include:
- Rotary encoders
- Touchpads
- Direct input pins
- Key matrices

Each input device implements the `InputDevice` trait, which requires:
- An associated `EventType` that defines what kind of events this device generates
- A `run()` method containing the device's main logic
- An `event_sender()` method to get the channel for sending events

Example implementation:

```rust
struct MyEncoder;
impl InputDevice for MyEncoder {
    type EventType = EncoderEvent;
    async fn run(&mut self) {
        // Read encoder and send events
        let event = EncoderEvent::Clockwise;
        self.event_sender().send(event).await;
    }
}
```

## Input Processors 

Input processors handle events from input devices and convert them into HID reports. A processor:

- Receives events from one or more input devices
- Processes those events into HID reports
- Sends the reports to USB/BLE

The `InputProcessor` trait defines this behavior with:

- Associated types for the events it handles and reports it generates
- A `process()` method to convert events to reports
- Channel getters for receiving events and sending reports

<!-- 
## Running Multiple Devices

RMK provides macros to run multiple input devices and processors concurrently:
```rust
// Run multiple input devices
let encoder = MyEncoder::new();
let touchpad = MyTouchpad::new();

// Run multiple processors
let encoder_proc = EncoderProcessor::new();
let touchpad_proc = TouchpadProcessor::new();

join(run_processors!(encoder_proc, touchpad_proc), run_devices!(encoder, touchpad)).await;
``` -->
