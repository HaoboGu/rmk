# Plan: Refactor InputDevice/InputProcessor/Controller Macro System

## Summary

Refactor the RMK macro system to:
1. Add `Runnable` as supertrait to `InputDevice`, `InputProcessor`, `Controller`
2. Add new `#[input_device]` macro
3. Generate `Runnable` implementations in macros
4. Support combined `Runnable` for structs with multiple attributes (e.g., `#[input_device]` + `#[controller]`)

## Current State

### Traits
- `Runnable`: Base trait with `async fn run(&mut self) -> !`
- `InputDevice`: Has `read_event() -> !`, no automatic `Runnable` impl
- `InputProcessor`: Requires `Runnable` as supertrait, macro generates both `Runnable` and `InputProcessor` impl
- `Controller`: Has `process_event()` and `next_message()`, `EventController` provides `event_loop()` via blanket impl

### Macros
- `#[input_processor(subscribe = [...])]` - generates `Runnable` + `InputProcessor` impl
- `#[controller(subscribe = [...])]` - generates `Controller` impl only (uses `event_loop()` from `EventController`)

## Design Decisions (Confirmed)

1. **Marker attribute approach**: First macro to expand adds `#[runnable_generated]` marker. Later macros see the marker and skip `Runnable` generation.

2. **Macro workflow**:
   - Each macro checks for `#[runnable_generated]` marker
   - If marker NOT present:
     - Check for other macro attributes to determine combination scenario
     - Generate `Runnable` (simple or combined)
     - Add `#[runnable_generated]` marker
     - Preserve other macro attributes
   - If marker present:
     - Skip `Runnable` generation
     - Only generate own trait impl
     - Remove own attribute

3. **Error handling**: Generate compile-time error if `#[input_device]` + `#[input_processor]` are combined (mutually exclusive)

4. **Concurrency**: Use true concurrent execution with `select_biased!` for combined structs

5. **InputDevice::read_event() return type**:
   - Single event device: returns the event type directly (e.g., `BatteryEvent`)
   - Multi-event device: user defines their own enum, variant names must match type names (e.g., `BatteryEvent(BatteryEvent)`)

6. **Trait hierarchy**: All three traits (`InputDevice`, `InputProcessor`, `Controller`) extend `Runnable` as supertrait. The macros generate the `Runnable` implementation.

7. **EventController blanket impl**: KEEP the blanket impl `impl<T: Controller> EventController for T {}`.

8. **Combined Runnable uses trait methods**: Call `self.process_event()` instead of direct handler methods, call `PollingController::update()` instead of `self.poll()`.

## Implementation Steps

### Phase 1: Trait Modifications

**File**: [rmk/src/input_device/mod.rs](rmk/src/input_device/mod.rs)

1. Change `InputDevice` trait to return event and extend `Runnable`:
```rust
// Before
pub trait InputDevice {
    async fn read_event(&mut self) -> !;
}

// After
pub trait InputDevice: Runnable {
    type Event;
    async fn read_event(&mut self) -> Self::Event;
}
```

2. Keep `InputProcessor` extending `Runnable` (already does this):
```rust
pub trait InputProcessor<...>: Runnable { ... }
```

**File**: [rmk/src/controller/mod.rs](rmk/src/controller/mod.rs)

3. Make `Controller` trait extend `Runnable`:
```rust
// Before
pub trait Controller {
    type Event;
    async fn process_event(&mut self, event: Self::Event);
    async fn next_message(&mut self) -> Self::Event;
}

// After
pub trait Controller: Runnable {
    type Event;
    async fn process_event(&mut self, event: Self::Event);
    async fn next_message(&mut self) -> Self::Event;
}
```

4. **KEEP** `EventController` blanket impl - unchanged:
```rust
impl<T: Controller> EventController for T {}
```

### Phase 2: Add `#[input_device]` Macro

**New file**: [rmk-macro/src/input/device.rs](rmk-macro/src/input/device.rs)

#### Single-event device syntax:
```rust
#[input_device(publish = [BatteryEvent])]
pub struct BatteryReader { ... }

impl BatteryReader {
    // Returns the event type directly
    async fn read_event(&mut self) -> BatteryEvent { ... }
}
```

#### Multi-event device syntax:
```rust
// User-defined event enum
// IMPORTANT: variant names must match type names
pub enum MyDeviceEvent {
    BatteryEvent(BatteryEvent),
    PointingEvent(PointingEvent),
}

#[input_device(publish = [BatteryEvent, PointingEvent], event = MyDeviceEvent)]
#[controller(subscribe = [SomeEvent])]  // optional
pub struct MyDevice { ... }

impl MyDevice {
    // Returns the user-defined enum
    async fn read_event(&mut self) -> MyDeviceEvent { ... }
}
```

Create `input_device_impl()` function that:
- Parses the struct and `publish = [EventType, ...]` attribute (and optional `event = EnumType`)
- Determines if single-event or multi-event based on:
  - Single-event: `publish` has one element and no `event` parameter
  - Multi-event: `publish` has multiple elements and `event` parameter is required
- Check for `#[runnable_generated]` marker:
  - **If NOT present**:
    - Check for `#[controller]` attribute
    - Generate `Runnable` (combined if controller exists)
    - Add `#[runnable_generated]` marker to output
    - Preserve `#[controller]` attribute (if present)
  - **If present**:
    - Skip `Runnable` generation
- Implements `InputDevice` trait:
  - Single-event: `type Event = TheEventType`
  - Multi-event: `type Event = UserDefinedEnum`
- Remove `#[input_device]` attribute from output

**Example: Single-event `#[input_device]`**:
```rust
// Input:
#[input_device(publish = [BatteryEvent])]
pub struct BatteryReader { ... }

// Output:
#[runnable_generated]
pub struct BatteryReader { ... }

impl InputDevice for BatteryReader {
    type Event = BatteryEvent;
    // read_event() is implemented by user
}

impl Runnable for BatteryReader {
    async fn run(&mut self) -> ! {
        loop {
            let event = self.read_event().await;
            publish_input_event_async(event).await;
        }
    }
}
```

**Example: Multi-event `#[input_device]`**:
```rust
// Input:
pub enum MyDeviceEvent {
    BatteryEvent(BatteryEvent),
    PointingEvent(PointingEvent),
}

#[input_device(publish = [BatteryEvent, PointingEvent], event = MyDeviceEvent)]
pub struct MyDevice { ... }

// Output:
#[runnable_generated]
pub struct MyDevice { ... }

impl InputDevice for MyDevice {
    type Event = MyDeviceEvent;
}

impl Runnable for MyDevice {
    async fn run(&mut self) -> ! {
        loop {
            let event = self.read_event().await;
            match event {
                MyDeviceEvent::BatteryEvent(e) => publish_input_event_async(e).await,
                MyDeviceEvent::PointingEvent(e) => publish_input_event_async(e).await,
            }
        }
    }
}
```

**Example: `#[input_device]` + `#[controller]` combined**:
```rust
// Input:
#[input_device(publish = [BatteryEvent])]
#[controller(subscribe = [SomeEvent])]
pub struct MyDevice { ... }

// Output:
#[runnable_generated]  // Added marker
#[controller(subscribe = [SomeEvent])]  // Preserved for next macro
pub struct MyDevice { ... }

impl InputDevice for MyDevice { ... }
impl Runnable for MyDevice {
    // Combined Runnable (reads controller attr to know what to combine)
}
```

**Example: `#[controller]` expands first (no marker), then `#[input_device]`**:
```rust
// Step 1 - controller expands first:
// Input:
#[input_device(publish = [BatteryEvent])]
#[controller(subscribe = [SomeEvent])]
pub struct MyDevice { ... }

// Output after controller:
#[runnable_generated]  // Added marker
#[input_device(publish = [BatteryEvent])]  // Preserved
pub struct MyDevice { ... }

impl Controller for MyDevice { ... }
impl Runnable for MyDevice {
    // Combined Runnable (reads input_device attr to know what to combine)
}

// Step 2 - input_device expands:
// Input (sees marker):
#[runnable_generated]
#[input_device(publish = [BatteryEvent])]
pub struct MyDevice { ... }

// Output after input_device:
#[runnable_generated]
pub struct MyDevice { ... }

impl InputDevice for MyDevice { ... }
// NO Runnable - marker present
```

**File**: [rmk-macro/src/lib.rs](rmk-macro/src/lib.rs)

Export the new macro:
```rust
#[proc_macro_attribute]
pub fn input_device(attr: TokenStream, item: TokenStream) -> TokenStream {
    input::device::input_device_impl(attr, item)
}
```

### Phase 3: Update `#[input_processor]` Macro

**File**: [rmk-macro/src/input/processor.rs](rmk-macro/src/input/processor.rs)

1. Check for `#[runnable_generated]` marker
2. **If NOT present**:
   - Check for `#[controller]` attribute
   - Generate `Runnable` (combined if controller exists)
   - Add `#[runnable_generated]` marker
   - Preserve `#[controller]` attribute
3. **If present**:
   - Skip `Runnable` generation
4. Always generate `InputProcessor` trait impl
5. Remove `#[input_processor]` attribute

### Phase 4: Update `#[controller]` Macro

**File**: [rmk-macro/src/controller/mod.rs](rmk-macro/src/controller/mod.rs)

1. Check for `#[runnable_generated]` marker
2. **If NOT present**:
   - Check for `#[input_device]` or `#[input_processor]` attributes
   - Generate `Runnable` (combined if other attrs exist, standalone otherwise)
   - Add `#[runnable_generated]` marker
   - Preserve `#[input_device]`/`#[input_processor]` attributes
3. **If present**:
   - Skip `Runnable` generation
4. Always generate `Controller` trait impl
5. Remove `#[controller]` attribute

**Standalone controller Runnable**:
```rust
impl Runnable for MyController {
    async fn run(&mut self) -> ! {
        self.event_loop().await  // Uses EventController blanket impl
    }
}
```

**Standalone controller with poll_interval**:
```rust
impl Runnable for MyController {
    async fn run(&mut self) -> ! {
        self.polling_loop().await  // Uses PollingController
    }
}
```

### Phase 5: Combined Runnable Generation

When generating combined Runnable, the macro must:
1. Parse the other macro's attribute to extract its configuration
2. Generate appropriate `select_biased!` arms for all components

**input_device + controller combination (single-event)**:
```rust
impl Runnable for MyDevice {
    async fn run(&mut self) -> ! {
        use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
        use ::futures::FutureExt;
        use ::rmk::controller::Controller;

        let mut ctrl_sub0 = <CtrlEvent0 as ControllerEvent>::controller_subscriber();

        loop {
            ::futures::select_biased! {
                event = self.read_event().fuse() => {
                    publish_input_event_async(event).await;
                },
                ctrl_event = ctrl_sub0.next_event().fuse() => {
                    <Self as Controller>::process_event(self, ctrl_event.into()).await;
                },
            }
        }
    }
}
```

**input_device + controller combination (multi-event)**:
```rust
impl Runnable for MyDevice {
    async fn run(&mut self) -> ! {
        use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
        use ::futures::FutureExt;
        use ::rmk::controller::Controller;

        let mut ctrl_sub0 = <CtrlEvent0 as ControllerEvent>::controller_subscriber();

        loop {
            ::futures::select_biased! {
                event = self.read_event().fuse() => {
                    match event {
                        MyDeviceEvent::BatteryEvent(e) => publish_input_event_async(e).await,
                        MyDeviceEvent::PointingEvent(e) => publish_input_event_async(e).await,
                    }
                },
                ctrl_event = ctrl_sub0.next_event().fuse() => {
                    <Self as Controller>::process_event(self, ctrl_event.into()).await;
                },
            }
        }
    }
}
```

**input_device + controller with poll_interval (single-event)**:
```rust
impl Runnable for MyDevice {
    async fn run(&mut self) -> ! {
        use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
        use ::futures::FutureExt;
        use ::rmk::controller::{Controller, PollingController};

        let mut ctrl_sub0 = <CtrlEvent0 as ControllerEvent>::controller_subscriber();
        let mut last = ::embassy_time::Instant::now();

        loop {
            let elapsed = last.elapsed();
            let interval = <Self as PollingController>::interval(self);
            let timer = ::embassy_time::Timer::after(
                interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
            );

            ::futures::select_biased! {
                event = self.read_event().fuse() => {
                    publish_input_event_async(event).await;
                },
                ctrl_event = ctrl_sub0.next_event().fuse() => {
                    <Self as Controller>::process_event(self, ctrl_event.into()).await;
                },
                _ = timer.fuse() => {
                    <Self as PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                },
            }
        }
    }
}
```

**input_device + controller with poll_interval (multi-event)**:
```rust
impl Runnable for MyDevice {
    async fn run(&mut self) -> ! {
        use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
        use ::futures::FutureExt;
        use ::rmk::controller::{Controller, PollingController};

        let mut ctrl_sub0 = <CtrlEvent0 as ControllerEvent>::controller_subscriber();
        let mut last = ::embassy_time::Instant::now();

        loop {
            let elapsed = last.elapsed();
            let interval = <Self as PollingController>::interval(self);
            let timer = ::embassy_time::Timer::after(
                interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
            );

            ::futures::select_biased! {
                event = self.read_event().fuse() => {
                    match event {
                        MyDeviceEvent::BatteryEvent(e) => publish_input_event_async(e).await,
                        MyDeviceEvent::PointingEvent(e) => publish_input_event_async(e).await,
                    }
                },
                ctrl_event = ctrl_sub0.next_event().fuse() => {
                    <Self as Controller>::process_event(self, ctrl_event.into()).await;
                },
                _ = timer.fuse() => {
                    <Self as PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                },
            }
        }
    }
}
```

### Phase 6: Update Existing Implementations

Update existing `InputDevice` implementations:
- [rmk/src/input_device/battery.rs](rmk/src/input_device/battery.rs) - `ChargingStateReader`
- NrfAdc, Matrix, DirectPin, RotaryEncoder, PMW3610, etc.

Example migration for `ChargingStateReader` (single event type):
```rust
// Before
impl<I: InputPin> InputDevice for ChargingStateReader<I> {
    async fn read_event(&mut self) -> ! {
        loop {
            // ... internal loop with publish
        }
    }
}

// After
#[input_device(publish = [ChargingStateEvent])]
pub struct ChargingStateReader<I: InputPin> { ... }

impl<I: InputPin> ChargingStateReader<I> {
    async fn read_event(&mut self) -> ChargingStateEvent {
        // Wait and return single event
        embassy_time::Timer::after_secs(5).await;
        ChargingStateEvent { charging: self.state_input.is_low().unwrap_or(false) }
    }
}
```

Example for multi-event device:
```rust
// User defines enum with variant names matching type names
pub enum MyDeviceEvent {
    BatteryEvent(BatteryEvent),
    PointingEvent(PointingEvent),
}

#[input_device(publish = [BatteryEvent, PointingEvent], event = MyDeviceEvent)]
pub struct MultiSensorDevice { ... }

impl MultiSensorDevice {
    async fn read_event(&mut self) -> MyDeviceEvent {
        // User decides which event to return based on internal state
        if self.battery_changed() {
            MyDeviceEvent::BatteryEvent(BatteryEvent { ... })
        } else {
            MyDeviceEvent::PointingEvent(PointingEvent { ... })
        }
    }
}
```

## Critical Files

| File | Changes |
|------|---------|
| [rmk/src/input_device/mod.rs](rmk/src/input_device/mod.rs) | Change `InputDevice` to extend `Runnable`, change `read_event()` return type |
| [rmk/src/controller/mod.rs](rmk/src/controller/mod.rs) | Make `Controller` extend `Runnable`, KEEP `EventController` blanket impl |
| [rmk-macro/src/input/device.rs](rmk-macro/src/input/device.rs) | New file: `#[input_device]` macro implementation |
| [rmk-macro/src/input/processor.rs](rmk-macro/src/input/processor.rs) | Add marker detection, generate combined `Runnable` |
| [rmk-macro/src/controller/mod.rs](rmk-macro/src/controller/mod.rs) | Add marker detection, generate combined `Runnable` |
| [rmk-macro/src/lib.rs](rmk-macro/src/lib.rs) | Export `#[input_device]` macro |
| [rmk/src/input_device/battery.rs](rmk/src/input_device/battery.rs) | Update `ChargingStateReader` to new pattern |
| Other input devices | NrfAdc, Matrix, DirectPin, RotaryEncoder, PMW3610, etc. |

## Verification

1. **Build check**: `cargo build --all-features` in rmk workspace
2. **Test existing examples**: Ensure `BatteryProcessor` and `KeyboardIndicatorController` still work
3. **Test new `#[input_device]` macro**: Apply to `ChargingStateReader`
4. **Test multi-event device**: Create a device publishing multiple event types
5. **Test combined usage**: Create a test struct with both `#[input_device]` + `#[controller]` attributes
6. **Test macro order**: Verify both `#[input_device] #[controller]` and `#[controller] #[input_device]` work correctly
7. **Run unit tests**: `cargo test` in rmk-macro crate
