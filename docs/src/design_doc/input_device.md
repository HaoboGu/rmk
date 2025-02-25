# Input devices

The definition of input devices varies, but for RMK, we focus on two categories: keys and sensors.

- Keys are straightforward—they are essentially switches with two states (pressed/released).
- Sensors are more complex devices that can produce various types of data, such as joysticks, mice, trackpads, and trackballs.

RMK's input device framework is designed to provide a simple yet extensible way to handle both keys and sensors. Below is an overview of the framework:

![input_device_framework](../images/input_device_framework.excalidraw.svg)

## Input device trait

The input devices can be key matrix or sensors, which read the physical devices, send raw events to the input processors. All input devices in RMK should implement the `InputDevice` trait:

All input devices in RMK should implement the `InputDevice` trait:

```rust
pub trait InputDevice {
    /// Event type that input device will send
    type EventType;

    /// Starts the input device task.
    ///
    /// This asynchronous method should contain the main logic for the input device.
    /// It will be executed concurrently with other input devices using the run_devices macro.
    fn run(&mut self) -> impl Future<Output = ()>;

    /// Get the event sender for the input device. All events should be sent through this channel.
    fn event_sender(&self) -> Sender<RawMutex, Self::EventType, EVENT_CHANNEL_SIZE>;
}
}
```

This trait should be used with the `run_devices!` macro:

```rust
// Suppose that the d1 & d2 both implement `InputDevice`. `run()` will be called in `run_devices!`
run_devices!(d1, d2).await;
```

> Why `run_devices!`?
>
> Currently, embassy-rs does not support generic tasks. The only option is to join all tasks (the `run` functions in `InputDevice`) together. That's what `run_devices!` does.

## Event Types

Each input device defines its own `EventType`. RMK provides a default `Event` enum that is compatible with built-in `InputProcessor`s:

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
    /// The event type that the input processor receives
    type EventType;
    
    /// The report type that the input processor sends
    type ReportType;

    /// Process incoming events and convert them to HID reports
    fn process(&mut self, event: Self::EventType) -> impl Future<Output = ()>;

    /// Get the input event channel receiver
    fn event_receiver(&self) -> Receiver<RawMutex, Self::EventType, EVENT_CHANNEL_SIZE>;

    /// Get the output report sender for the input processor.
    ///
    /// The input processor sends keyboard reports to this channel.
    fn report_sender(
        &self,
    ) -> Sender<RawMutex, Self::ReportType, REPORT_CHANNEL_SIZE>;

    /// Default implementation of the input processor. It wait for a new event from the event channel,
    /// then process the event.
    ///
    /// The report is sent to the USB/BLE in the `process` method.
    fn run(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                let event = self.event_receiver().receive().await;
                self.process(event).await;
            }
        }
    }
}
```


The `process` method is responsible for processing input events and sending HID reports through the report channel. All processors share a common keymap state through `&'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>`.

This design balances convenience and flexibility:
- For common devices, developers can use the built-in `Event` and `InputProcessor` implementations
- For advanced use cases, developers can define custom events and processors to fully control the input logic
