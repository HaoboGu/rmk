# Input devices

The definition of input devices varies, but for RMK, we focus on two categories: keys and sensors.

- Keys are straightforward—they are essentially switches with two states (pressed/released).
- Sensors are more complex devices that can produce various types of data, such as joysticks, mice, trackpads, and trackballs.

RMK's input device framework is designed to provide a simple yet extensible way to handle both keys and sensors. Below is an overview of the framework:

![input_device_framework](../images/input_device_framework.excalidraw.svg)

## Input device trait

Input devices such as key matrices or sensors read physical devices and generate events. All input devices in RMK should implement the `InputDevice` trait:

```rust
pub trait InputDevice {
    /// Read the raw input event
    async fn read_event(&mut self) -> Event;
}
```

This trait is used with the `run_devices!` macro to collect events from multiple input devices and send them to a specified channel:

```rust
// Send events from matrix to EVENT_CHANNEL
run_devices! (
    (matrix) => EVENT_CHANNEL,
)
```

> Why `run_devices!`?
>
> Currently, embassy-rs does not support generic tasks. The only option is to join all tasks together to handle multiple input devices concurrently. The `run_devices!` macro helps accomplish this efficiently.

## Runnable trait

For components that need to run continuously in a task, RMK provides the `Runnable` trait:

```rust
pub trait Runnable {
    async fn run(&mut self);
}
```

The `Keyboard` type implements this trait to process events and generate reports.

## Event Types

RMK provides a default `Event` enum that is compatible with built-in `InputProcessor`s:

```rust
#[non_exhaustive]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Event {
    /// Keyboard event
    Key(KeyEvent),
    /// Rotary encoder, ec11 compatible models
    RotaryEncoder(RotaryEncoderEvent),
    /// Multi-touch touchpad
    Touchpad(TouchpadEvent),
    /// Joystick, suppose we have x,y,z axes for this joystick
    Joystick([AxisEvent; 3]),
    /// An AxisEvent in a stream of events. The receiver should keep receiving events until it receives [`Eos`] event.
    AxisEventStream(AxisEvent),
    /// End of the event sequence
    ///
    /// This is used with [`AxisEventStream`] to indicate the end of the event sequence.
    Eos,
}
```

The `Event` enum aims to cover raw outputs from common input devices. It also provides a stream-like axis event representation via `AxisEventStream` for devices with a variable number of axes. When using `AxisEventStream`, the `Eos` event must be sent to indicate the end of the sequence.

## Input Processor Trait

Input processors receive events from input devices, process them, and convert the results into HID reports for USB/BLE transmission. All input processors must implement the `InputProcessor` trait:

```rust
pub trait InputProcessor {
    /// Process the incoming events, convert them to HID report [`Report`],
    /// then send the report to the USB/BLE.
    ///
    /// Note there might be multiple HID reports are generated for one event,
    /// so the "sending report" operation should be done in the `process` method.
    /// The input processor implementor should be aware of this.  
    async fn process(&mut self, event: Event);

    /// Send the processed report.
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }
}
```

The `process` method is responsible for processing input events and sending HID reports through the report channel. All processors share a common keymap state through `&'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>`.

## Standard System Architecture

RMK uses a standard pattern for running the entire system:

```rust
// Start the system with three concurrent tasks
join3(
    // Task 1: Run all input devices and send events to EVENT_CHANNEL
    run_devices! (
        (matrix) => EVENT_CHANNEL,
    ),
    // Task 2: Run the keyboard processor
    keyboard.run(),
    // Task 3: Run the main RMK system
    run_rmk(&keymap, driver, storage, light_controller, rmk_config),
)
.await;
```

This design balances convenience and flexibility:
- For common devices, developers can use the built-in `Event` types and RMK's processing pipeline
- For advanced use cases, developers can define custom events and processors to fully control the input logic
- The keyboard is special -- it receives events only from `KEY_EVENT_CHANNEL` and processes `KeyEvent`s only. `KeyEvent` from ALL devices are handled by the `Keyboard` processor, then the other events are dispatched to binded processors.