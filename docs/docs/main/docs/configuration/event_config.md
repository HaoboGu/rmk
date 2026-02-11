# Event Configuration

RMK provides configuration that allows you to tune the event system based on your specific needs and hardware constraints.

## Overview

All [built-in events](../features/processor.md#built-in-events) use Embassy's `PubSubChannel` for efficient inter-task communication. RMK's configuration system allows you to customize three key parameters for each event type:

- **`channel_size`**: Buffer size for the event channel - how many events can be queued
- **`pubs`**: Number of publishers - how many concurrent tasks can publish this event type
- **`subs`**: Number of subscribers - how many concurrent tasks can subscribe to this event type

Each event in RMK has default values chosen for typical use cases. You only need to configure events when your specific requirements differ from these defaults.

## Configuration Method

Add an `[event]` section to your `keyboard.toml` file. You can configure any subset of the three parameters for each event - unspecified parameters will use their default values.

**Configuration syntax:**

```toml
[event]
# Configure specific parameters for individual events
event_name.channel_size = <value>
event_name.pubs = <value>
event_name.subs = <value>
```

**Examples:**

```toml
[event]
# Increase key event buffer for fast typing
key.channel_size = 16

# Add more subscribers for multiple displays monitoring layer changes
layer_change.subs = 8

# Reduce subscribers to save memory on constrained devices
battery_state.subs = 2
led_indicator.subs = 2

# Configure multiple parameters for one event
peripheral_battery.channel_size = 4
peripheral_battery.subs = 4
```

## Configurable Events

All [built-in events](../features/processor.md#built-in-events) can be configured. Here's the complete list showing the mapping between configuration names and event types:

### BLE Events
- `ble_state_change` → [`BleStateChangeEvent`](../features/processor.md#built-in-events) - BLE connection state changes (advertising, connected, disconnected)
- `ble_profile_change` → [`BleProfileChangeEvent`](../features/processor.md#built-in-events) - BLE profile switching

### Connection Events
- `connection_change` → [`ConnectionChangeEvent`](../features/processor.md#built-in-events) - USB/BLE connection type changes

### Input Events
- `key` → [`KeyEvent`](../features/processor.md#built-in-events) - Key press/release events (default: channel_size=8 for fast typing)
- `modifier` → [`ModifierEvent`](../features/processor.md#built-in-events) - Modifier key state changes (Shift, Ctrl, Alt, etc.)
- `keyboard` → [`KeyboardEvent`](../features/processor.md#built-in-events) - Raw keyboard events (default: channel_size=16 for input buffering)
- `pointing` → [`PointingEvent`](../features/processor.md#built-in-events) - Mouse/pointing device movement events (default: channel_size=8)
- `touchpad` → [`TouchpadEvent`](../features/processor.md#built-in-events) - Multi-touch touchpad events (default: channel_size=8)

### Keyboard State Events
- `layer_change` → [`LayerChangeEvent`](../features/processor.md#built-in-events) - Active layer changes (default: subs=4 for multiple displays)
- `wpm_update` → [`WpmUpdateEvent`](../features/processor.md#built-in-events) - WPM statistics updates
- `led_indicator` → [`LedIndicatorEvent`](../features/processor.md#built-in-events) - LED indicator state changes (Caps Lock, Num Lock, Scroll Lock)
- `sleep_state` → [`SleepStateEvent`](../features/processor.md#built-in-events) - Sleep/wake state transitions

### Power Events
- `battery_state` → [`BatteryStateEvent`](../features/processor.md#built-in-events) - Battery state changes (includes level and charging status, default: subs=4 for multiple displays)
- `battery_adc` → [`BatteryAdcEvent`](../features/processor.md#built-in-events) - Raw battery ADC readings (default: channel_size=2)
- `charging_state` → [`ChargingStateEvent`](../features/processor.md#built-in-events) - Charging state changes (default: channel_size=2)

### Split Keyboard Events
- `peripheral_connected` → [`PeripheralConnectedEvent`](../features/processor.md#built-in-events) - Peripheral connection status
- `central_connected` → [`CentralConnectedEvent`](../features/processor.md#built-in-events) - Central connection status
- `peripheral_battery` → [`PeripheralBatteryEvent`](../features/processor.md#built-in-events) - Peripheral battery state updates (default: channel_size=2, subs=2)
- `clear_peer` → [`ClearPeerEvent`](../features/processor.md#built-in-events) - BLE peer clearing events

For detailed information about these event types and how to use them in custom processors, see the [Processor](../features/processor.md) documentation.
