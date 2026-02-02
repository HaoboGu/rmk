# Plan: Refactor InputDevice/InputProcessor/Controller Macro System

## Summary

Refactor the RMK macro system to:
1. Add new `#[input_device]` macro with `read_xxx_event()` pattern (single-event only)
2. Add new `#[derive(InputEvent)]` macro for multi-event enum auto-dispatch
3. Generate `Runnable` implementations in all macros
4. Support combined `Runnable` for structs with multiple attributes (e.g., `#[input_device]` + `#[controller]`)
5. Define `#[rmk::runnable_generated]` marker attribute
6. Remove `run_devices!` and `bind_device_and_processor_and_run!` macros

## Current State

### Traits (Already Updated)
- `Runnable`: Base trait with `async fn run(&mut self) -> !`
- `InputDevice`: Extends `Runnable`, has `read_event() -> !` (needs change to return `Self::Event`)
- `InputProcessor`: Extends `Runnable`, macro generates both `Runnable` and `InputProcessor` impl
- `Controller`: Extends `Runnable`, `EventController` provides `event_loop()` via blanket impl

### Macros
- `#[input_processor(subscribe = [...])]` - generates `Runnable` + `InputProcessor` impl
- `#[controller(subscribe = [...])]` - generates `Controller` impl only (needs to add `Runnable` generation)
- `run_devices!` - assumes `read_event() -> !` (needs removal)
- `bind_device_and_processor_and_run!` - assumes `read_event() -> !` (needs removal)

## Design Decisions (Confirmed)

1. **Marker attribute approach**: Define `#[rmk::runnable_generated]` as a real attribute. Always emit as `#[::rmk::runnable_generated]` (fully qualified). Detection checks for both `runnable_generated` and `rmk::runnable_generated`.

2. **Macro workflow**:
   - Each macro checks for `#[runnable_generated]` or `#[rmk::runnable_generated]` marker
   - If marker NOT present:
     - Check for other macro attributes to determine combination scenario
     - Generate `Runnable` (simple or combined)
     - Add `#[::rmk::runnable_generated]` marker **AFTER** other macro attributes (to ensure it's expanded last)
     - Preserve other macro attributes
   - If marker present:
     - Skip `Runnable` generation
     - Only generate own trait impl
     - Remove own attribute

3. **Marker attribute ordering**: The marker attribute MUST be placed **AFTER** other proc-macro attributes in the generated output. This ensures the marker is expanded last, preventing it from being consumed before other macros can detect it. Example:
   ```rust
   // CORRECT - marker is last, expanded last
   #[controller(subscribe = [SomeEvent])]
   #[::rmk::runnable_generated]
   pub struct MyDevice { ... }

   // WRONG - marker is first, would be expanded away before #[controller] runs
   #[::rmk::runnable_generated]
   #[controller(subscribe = [SomeEvent])]
   pub struct MyDevice { ... }
   ```

4. **Error handling**: Use `syn::Error::new_spanned(...).to_compile_error()` for invalid combinations (not panic) to provide actionable error messages.

6. **Concurrency**: Use `select_biased!` everywhere (required for `no_std` environments; `select!` is not available).

7. **InputDevice pattern** (single-event only):
   - `#[input_device]` macro supports **single-event devices only**
   - User implements `read_xxx_event() -> XXXEvent` inherent method
   - For multi-event devices, use `#[derive(InputEvent)]` on a user-defined enum

8. **Multi-event enum dispatch** (`#[derive(InputEvent)]`):
   - User defines their own enum with variants wrapping `InputEvent` types
   - Derive macro generates `publish()` method that auto-dispatches to correct channel
   - Derive macro generates `From<EventType>` impls for each variant
   - User implements `InputDevice` and `Runnable` manually, using `event.publish().await`

9. **Trait hierarchy**: All three traits (`InputDevice`, `InputProcessor`, `Controller`) already extend `Runnable` as supertrait. The macros generate the `Runnable` implementation.

10. **EventController blanket impl**: KEEP the blanket impl `impl<T: Controller> EventController for T {}`.

11. **Combined Runnable uses trait methods**: Call `self.process_event()` instead of direct handler methods, call `PollingController::update()` instead of `self.poll()`.

12. **Controller event enum**: Generate `From<EventType>` impls for each subscribed event type to the controller's event enum.

13. **Remove old macros**: `run_devices!` and `bind_device_and_processor_and_run!` are removed entirely. Users should use `run_all!` with `Runnable::run()`.

## Implementation Steps

### Phase 0: Define Marker Attribute

**File**: [rmk/src/lib.rs](rmk/src/lib.rs) or [rmk/src/input_device/mod.rs](rmk/src/input_device/mod.rs)

Define the marker attribute:
```rust
/// Marker attribute indicating Runnable impl has been generated.
/// This is used internally by macros to coordinate Runnable generation.
#[doc(hidden)]
pub use rmk_macro::runnable_generated;
```

**File**: [rmk-macro/src/lib.rs](rmk-macro/src/lib.rs)

```rust
/// Marker attribute for coordinating Runnable generation between macros.
/// Do not use directly.
#[doc(hidden)]
#[proc_macro_attribute]
pub fn runnable_generated(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item  // Pass through unchanged
}
```

### Phase 1: Trait Modifications & Macro Deprecation

**File**: [rmk/src/input_device/mod.rs](rmk/src/input_device/mod.rs)

1. Change `InputDevice` trait to return event (already extends `Runnable`):
```rust
// Current
pub trait InputDevice: Runnable {
    async fn read_event(&mut self) -> !;
}

// After
pub trait InputDevice: Runnable {
    type Event;
    async fn read_event(&mut self) -> Self::Event;
}
```

2. Remove `run_devices!` and `bind_device_and_processor_and_run!` entirely:
```rust
// REMOVED - these macros no longer exist
// Users should migrate to `run_all!` with `Runnable::run()`
```

### Phase 2: Add `#[input_device]` Macro

**New file**: [rmk-macro/src/input/device.rs](rmk-macro/src/input/device.rs)

#### Single-event device syntax (only supported pattern):
```rust
#[input_device(publish = BatteryEvent)]
pub struct BatteryReader { ... }

impl BatteryReader {
    // User implements this inherent method
    async fn read_battery_event(&mut self) -> BatteryEvent {
        // Wait and return single event
        embassy_time::Timer::after_secs(5).await;
        BatteryEvent { level: self.read_level() }
    }
}
```

#### Multi-event devices (use `#[derive(InputEvent)]`):
```rust
// For multi-event devices, use #[derive(InputEvent)] on a user-defined enum
// See Phase 2.5 for details
#[derive(InputEvent)]
pub enum MultiDeviceEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}

pub struct MultiDevice { ... }

impl InputDevice for MultiDevice {
    type Event = MultiDeviceEvent;

    async fn read_event(&mut self) -> Self::Event {
        // User handles internal multiplexing
        ...
    }
}

impl Runnable for MultiDevice {
    async fn run(&mut self) -> ! {
        loop {
            let event = self.read_event().await;
            event.publish().await;  // Auto-dispatches to correct channel!
        }
    }
}
```

Create `input_device_impl()` function that:
- Parses the struct and `publish = EventType` attribute (single event type only)
- Validates only ONE event type is specified (compile error if array syntax used)
- Generates internal enum `{StructName}InputEventEnum` (single variant, not exposed to users)
- Check for `#[runnable_generated]` or `#[rmk::runnable_generated]` marker:
  - **If NOT present**:
    - Check for `#[controller]` attribute
    - Generate `Runnable` (combined if controller exists)
    - Add `#[::rmk::runnable_generated]` marker to output (fully qualified)
    - Preserve `#[controller]` attribute (if present)
  - **If present**:
    - Skip `Runnable` generation
- Check for `#[input_processor]` attribute → compile error (mutually exclusive)
- Implements `InputDevice` trait
- Remove `#[input_device]` attribute from output

**Example: `#[input_device]` output**:
```rust
// Input:
#[input_device(publish = BatteryEvent)]
pub struct BatteryReader { ... }

impl BatteryReader {
    async fn read_battery_event(&mut self) -> BatteryEvent { ... }
}

// Output:
#[::rmk::runnable_generated]
pub struct BatteryReader { ... }

// Internal enum (not exposed to users)
enum BatteryReaderInputEventEnum {
    Event0(BatteryEvent),
}

impl InputDevice for BatteryReader {
    type Event = BatteryReaderInputEventEnum;

    async fn read_event(&mut self) -> Self::Event {
        BatteryReaderInputEventEnum::Event0(self.read_battery_event().await)
    }
}

impl Runnable for BatteryReader {
    async fn run(&mut self) -> ! {
        use ::rmk::event::publish_input_event_async;
        loop {
            let event = self.read_battery_event().await;
            publish_input_event_async(event).await;
        }
    }
}
```

**Example: `#[input_device]` + `#[controller]` combined**:
```rust
// Input:
#[input_device(publish = BatteryEvent)]
#[controller(subscribe = [SomeEvent])]
pub struct MyDevice { ... }

// Output (marker placed AFTER other macro attributes):
#[controller(subscribe = [SomeEvent])]  // Preserved for next macro
#[::rmk::runnable_generated]  // Marker placed LAST (expanded last)
pub struct MyDevice { ... }

enum MyDeviceInputEventEnum { ... }

impl InputDevice for MyDevice { ... }
impl Runnable for MyDevice {
    // Combined Runnable (reads controller attr to know what to combine)
}
```

**File**: [rmk-macro/src/lib.rs](rmk-macro/src/lib.rs)

Export the new macro:
```rust
#[proc_macro_attribute]
pub fn input_device(attr: TokenStream, item: TokenStream) -> TokenStream {
    input::device::input_device_impl(attr, item)
}
```

### Phase 2.5: Add `#[derive(InputEvent)]` Macro for Multi-Event Enums

**New file**: [rmk-macro/src/input/event_derive.rs](rmk-macro/src/input/event_derive.rs)

This derive macro enables automatic event dispatch for user-defined multi-event enums.

#### Usage:
```rust
#[derive(InputEvent)]
pub enum MultiSensorEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}
```

#### Generated Code:
```rust
impl MultiSensorEvent {
    /// Publish this event to the appropriate channel based on variant
    pub async fn publish(self) {
        use ::rmk::event::publish_input_event_async;
        match self {
            MultiSensorEvent::Battery(e) => publish_input_event_async(e).await,
            MultiSensorEvent::Pointing(e) => publish_input_event_async(e).await,
        }
    }
}

// From impls for convenient construction
impl From<BatteryEvent> for MultiSensorEvent {
    fn from(e: BatteryEvent) -> Self {
        MultiSensorEvent::Battery(e)
    }
}

impl From<PointingEvent> for MultiSensorEvent {
    fn from(e: PointingEvent) -> Self {
        MultiSensorEvent::Pointing(e)
    }
}
```

#### Implementation:

Create `input_event_derive_impl()` function that:
- Validates input is an enum
- Validates all variants are tuple variants with exactly one field
- Extracts generics (`impl_generics`, `ty_generics`, `where_clause`) for generic enum support
- For each variant:
  - Extract variant name and inner type
  - Generate match arm for `publish()` method
  - Generate `From<InnerType>` impl (with generics)
- Generate the complete impl block with generics

```rust
pub fn input_event_derive_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Validate it's an enum
    let data_enum = match &input.data {
        syn::Data::Enum(e) => e,
        _ => return syn::Error::new_spanned(input, "#[derive(InputEvent)] can only be applied to enums")
            .to_compile_error()
            .into(),
    };

    let enum_name = &input.ident;
    let vis = &input.vis;

    // Extract generics for generic enum support
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Collect variant info
    let mut publish_arms = Vec::new();
    let mut from_impls = Vec::new();

    for variant in &data_enum.variants {
        let variant_name = &variant.ident;

        // Validate tuple variant with one field
        let inner_type = match &variant.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                &fields.unnamed.first().unwrap().ty
            }
            _ => return syn::Error::new_spanned(
                variant,
                "Each variant must be a tuple variant with exactly one field, e.g., `Battery(BatteryEvent)`"
            ).to_compile_error().into(),
        };

        // Generate publish match arm
        publish_arms.push(quote! {
            #enum_name::#variant_name(e) => ::rmk::event::publish_input_event_async(e).await
        });

        // Generate From impl with generics
        from_impls.push(quote! {
            impl #impl_generics From<#inner_type> for #enum_name #ty_generics #where_clause {
                fn from(e: #inner_type) -> Self {
                    #enum_name::#variant_name(e)
                }
            }
        });
    }

    let expanded = quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Publish this event to the appropriate channel based on variant
            #vis async fn publish(self) {
                match self {
                    #(#publish_arms),*
                }
            }
        }

        #(#from_impls)*
    };

    expanded.into()
}
```

**File**: [rmk-macro/src/lib.rs](rmk-macro/src/lib.rs)

Export the derive macro:
```rust
#[proc_macro_derive(InputEvent)]
pub fn input_event_derive(input: TokenStream) -> TokenStream {
    input::event_derive::input_event_derive_impl(input)
}
```

**File**: [rmk/src/lib.rs](rmk/src/lib.rs)

Re-export the derive macro:
```rust
pub use rmk_macro::InputEvent;
```

#### Complete Multi-Event Device Example:
```rust
use rmk::InputEvent;
use rmk::event::{BatteryEvent, PointingEvent};
use rmk::input_device::{InputDevice, Runnable};

#[derive(InputEvent)]
pub enum MultiSensorEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}

pub struct MultiSensorDevice {
    battery_signal: Signal<()>,
    motion_signal: Signal<()>,
    battery_level: u8,
    dx: i16,
    dy: i16,
}

impl InputDevice for MultiSensorDevice {
    type Event = MultiSensorEvent;

    async fn read_event(&mut self) -> Self::Event {
        use futures::{select_biased, FutureExt};

        select_biased! {
            _ = self.battery_signal.wait().fuse() => {
                BatteryEvent { level: self.battery_level }.into()
            },
            _ = self.motion_signal.wait().fuse() => {
                PointingEvent { dx: self.dx, dy: self.dy }.into()
            },
        }
    }
}

impl Runnable for MultiSensorDevice {
    async fn run(&mut self) -> ! {
        loop {
            let event = self.read_event().await;
            event.publish().await;  // Auto-dispatches!
        }
    }
}
```

### Phase 3: Update `#[input_processor]` Macro

**File**: [rmk-macro/src/input/processor.rs](rmk-macro/src/input/processor.rs)

1. Check for `#[runnable_generated]` or `#[rmk::runnable_generated]` marker
2. Check for `#[input_device]` attribute → compile error using `syn::Error` (mutually exclusive)
3. **If marker NOT present**:
   - Check for `#[controller]` attribute
   - Generate `Runnable` (combined if controller exists)
   - Add `#[::rmk::runnable_generated]` marker
   - Preserve `#[controller]` attribute
4. **If marker present**:
   - Skip `Runnable` generation
5. Always generate `InputProcessor` trait impl
6. Remove `#[input_processor]` attribute

### Phase 4: Update `#[controller]` Macro

**File**: [rmk-macro/src/controller/mod.rs](rmk-macro/src/controller/mod.rs)

1. Check for `#[runnable_generated]` or `#[rmk::runnable_generated]` marker
2. **If marker NOT present**:
   - Check for `#[input_device]` or `#[input_processor]` attributes
   - Generate `Runnable` (combined if other attrs exist, standalone otherwise)
   - Add `#[::rmk::runnable_generated]` marker
   - Preserve `#[input_device]`/`#[input_processor]` attributes
3. **If marker present**:
   - Skip `Runnable` generation
4. Always generate `Controller` trait impl
5. Generate `From<EventType>` impls for each subscribed event type
6. Remove `#[controller]` attribute

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

**Generated From impls for controller event enum**:
```rust
// For #[controller(subscribe = [EventA, EventB])]
pub enum MyControllerEventEnum {
    Event0(EventA),
    Event1(EventB),
}

impl From<EventA> for MyControllerEventEnum {
    fn from(e: EventA) -> Self {
        MyControllerEventEnum::Event0(e)
    }
}

impl From<EventB> for MyControllerEventEnum {
    fn from(e: EventB) -> Self {
        MyControllerEventEnum::Event1(e)
    }
}
```

### Phase 5: Combined Runnable Generation

When generating combined Runnable, the macro must:
1. Parse the other macro's attribute to extract its configuration
2. Generate appropriate `select_biased!` arms for all components

**input_device + controller combination**:
```rust
impl Runnable for MyDevice {
    async fn run(&mut self) -> ! {
        use ::rmk::event::{ControllerEvent, EventSubscriber, publish_input_event_async};
        use ::futures::FutureExt;
        use ::rmk::controller::Controller;

        let mut ctrl_sub0 = <CtrlEvent0 as ControllerEvent>::controller_subscriber();

        loop {
            ::futures::select_biased! {
                event = self.read_battery_event().fuse() => {
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

**input_device + controller with poll_interval**:
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
                event = self.read_battery_event().fuse() => {
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

### Phase 6: Update Existing Implementations

Update existing `InputDevice` implementations:
- [rmk/src/input_device/battery.rs](rmk/src/input_device/battery.rs) - `ChargingStateReader`
- NrfAdc, Matrix, DirectPin, RotaryEncoder, PMW3610, etc.

Example migration for `ChargingStateReader` (single event type - use macro):
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
#[input_device(publish = ChargingStateEvent)]
pub struct ChargingStateReader<I: InputPin> { ... }

impl<I: InputPin> ChargingStateReader<I> {
    // User implements this inherent method
    async fn read_charging_state_event(&mut self) -> ChargingStateEvent {
        // Wait and return single event
        embassy_time::Timer::after_secs(5).await;
        ChargingStateEvent { charging: self.state_input.is_low().unwrap_or(false) }
    }
}
```

Example for multi-event device (using `#[derive(InputEvent)]`):
```rust
use rmk::InputEvent;

// User defines their own enum with derive macro
#[derive(InputEvent)]
pub enum MultiSensorEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}

pub struct MultiSensorDevice {
    battery_signal: Signal<()>,
    motion_signal: Signal<()>,
    battery_level: u8,
    dx: i16,
    dy: i16,
}

impl InputDevice for MultiSensorDevice {
    type Event = MultiSensorEvent;

    async fn read_event(&mut self) -> Self::Event {
        use futures::{select_biased, FutureExt};

        // User handles internal multiplexing
        select_biased! {
            _ = self.battery_signal.wait().fuse() => {
                BatteryEvent { level: self.battery_level }.into()
            },
            _ = self.motion_signal.wait().fuse() => {
                PointingEvent { dx: self.dx, dy: self.dy }.into()
            },
        }
    }
}

impl Runnable for MultiSensorDevice {
    async fn run(&mut self) -> ! {
        loop {
            let event = self.read_event().await;
            event.publish().await;  // Auto-dispatches to correct channel!
        }
    }
}
```

## Method Naming Convention

For `#[input_device]` (single-event only):

Macro converts event type name to method name:
- `BatteryEvent` → `read_battery_event()`
- `ChargingStateEvent` → `read_charging_state_event()`
- `PointingEvent` → `read_pointing_event()`

Pattern: `read_{snake_case(EventTypeName without "Event" suffix)}_event()`

## Critical Files

| File | Changes |
|------|---------|
| [rmk/src/lib.rs](rmk/src/lib.rs) | Re-export `#[runnable_generated]` marker attribute and `#[derive(InputEvent)]` |
| [rmk/src/input_device/mod.rs](rmk/src/input_device/mod.rs) | Change `read_event()` return type, remove `run_devices!` and `bind_device_and_processor_and_run!` |
| [rmk-macro/src/lib.rs](rmk-macro/src/lib.rs) | Export `#[input_device]`, `#[runnable_generated]`, and `#[derive(InputEvent)]` macros |
| [rmk-macro/src/input/device.rs](rmk-macro/src/input/device.rs) | New file: `#[input_device]` macro implementation |
| [rmk-macro/src/input/event_derive.rs](rmk-macro/src/input/event_derive.rs) | New file: `#[derive(InputEvent)]` macro implementation |
| [rmk-macro/src/input/processor.rs](rmk-macro/src/input/processor.rs) | Add marker detection, generate combined `Runnable` |
| [rmk-macro/src/controller/mod.rs](rmk-macro/src/controller/mod.rs) | Add marker detection, generate `Runnable`, generate `From` impls |
| [rmk/src/input_device/battery.rs](rmk/src/input_device/battery.rs) | Update `ChargingStateReader` to new pattern |
| Other input devices | NrfAdc, Matrix, DirectPin, RotaryEncoder, PMW3610, etc. |

## Verification

1. **Build check**: `cd rmk && cargo build --release` in rmk workspace
2. **Test existing examples**: Ensure `BatteryProcessor` and `KeyboardIndicatorController` still work, run `cd rmk && cargo test --no-default-features --features "storage,std,vial,_ble"` until all tests pass
3. **Test new `#[input_device]` macro**: Apply to `ChargingStateReader`
4. **Test `#[derive(InputEvent)]`**: Create a multi-event enum and verify `publish()` and `From` impls work
5. **Test combined usage**: Create a test struct with both `#[input_device]` + `#[controller]` attributes
6. **Test macro order**: Verify both `#[input_device] #[controller]` and `#[controller] #[input_device]` work correctly
7. **Test invalid combinations**:
   - Verify `#[input_device]` + `#[input_processor]` produces compile error
   - Verify `#[input_device(publish = [A, B])]` (array syntax) produces compile error with helpful message
   - Verify `#[derive(InputEvent)]` on non-enum produces compile error
   - Verify `#[derive(InputEvent)]` on enum with non-tuple variants produces compile error
8. **Test From impls**: Verify controller event enum `From` impls work with `.into()`
9. **Test multi-event with derive**: Verify `#[derive(InputEvent)]` enum works with `InputDevice` + `Runnable`
10. **Test generic enum with derive**: Verify `#[derive(InputEvent)]` works on generic enums (e.g., `enum MyEvent<T> { ... }`)
11. **Run unit tests**: `cargo test` in rmk-macro crate
12. **Build examples**: First build `cd examples/use_rust/nrf52840_ble_split && cargo build --release` and `cd examples/use_config/nrf52840_ble_split && cargo build --release`, then build all examples using `sh scripts/clippy.sh && sh scripts/check_all.sh`

## Issue Fixes Summary

| Issue | Severity | Fix |
|-------|----------|-----|
| Multi-event `&mut self` borrow conflict | HIGH | `#[input_device]` is single-event only; multi-event uses `#[derive(InputEvent)]` for auto-dispatch |
| `run_devices!` / `bind_device_and_processor_and_run!` break | HIGH | Remove these macros entirely; users migrate to `run_all!` |
| Missing `From` impls for `ctrl_event.into()` | HIGH | Generate `From<EventType>` impls for controller event enum |
| Marker attribute ordering | HIGH | Place marker AFTER other macro attributes to ensure it's expanded last |
| `#[derive(InputEvent)]` drops generics | HIGH | Include `impl_generics`, `ty_generics`, `where_clause` in generated impls |
| Multi-event dispatch boilerplate | MEDIUM | `#[derive(InputEvent)]` generates `publish()` method for auto-dispatch |
| Marker attribute inconsistency | MEDIUM | Always emit `#[::rmk::runnable_generated]` (fully qualified); detect both forms |
| `select!` not available in no_std | MEDIUM | Use `select_biased!` everywhere |
| Panic vs compile_error! | LOW | Use `syn::Error::new_spanned(...).to_compile_error()` |

## Why `#[input_device]` Is Single-Event Only

The fundamental issue is Rust's borrow checker. When you have:

```rust
select_biased! {
    e = self.read_battery_event().fuse() => ...,
    e = self.read_joystick_event().fuse() => ...,
}
```

Both `self.read_battery_event()` and `self.read_joystick_event()` create futures that borrow `&mut self`. Even though only one future is polled at a time, the borrow checker sees both futures as potentially borrowing `&mut self` simultaneously, which violates Rust's borrowing rules.

**This is a fundamental Rust limitation, not a macro limitation.**

### Solution: `#[derive(InputEvent)]`

For multi-event devices, users define their own enum and use `#[derive(InputEvent)]`:

```rust
#[derive(InputEvent)]
pub enum MyDeviceEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}
```

This generates:
- `publish()` method for automatic dispatch to correct channel
- `From<EventType>` impls for convenient construction

Note that `MyDeviceEvent` itself doesn't have a static channel, it dispatches BatteryEvent and PointingEvent to their own channel.

And Users then implement a single `read_event()` method that returns their enum, avoiding the borrow conflict entirely.

