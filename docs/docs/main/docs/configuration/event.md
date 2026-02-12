# Event Configuration

RMK allows you to tune event channel parameters in `keyboard.toml` based on your specific needs and hardware constraints.

For an overview of events and how to define custom events, see the [Event](../features/event) documentation.

## Configuration Parameters

Each event channel has three configurable parameters:

- **`channel_size`**: Buffer size - how many events can be queued
- **`pubs`**: Number of publishers - how many concurrent tasks can publish
- **`subs`**: Number of subscribers - how many concurrent tasks can subscribe

Each event has default values for typical use cases. You can view all defaults in [`rmk-config/src/default_config/event_default.toml`](https://github.com/HaoboGu/rmk/blob/main/rmk-config/src/default_config/event_default.toml).

## Configuration Syntax

Add an `[event]` section to your `keyboard.toml`:

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
keyboard.channel_size = 16

# Add more subscribers for multiple displays monitoring layer changes
layer_change.subs = 8

# Reduce subscribers to save memory on constrained devices
battery_state.subs = 2
led_indicator.subs = 2

# Configure multiple parameters for one event
peripheral_battery.channel_size = 4
peripheral_battery.subs = 4
```

## Configurable Event Names

| Config Name | Event Type | Default Notes |
|-------------|------------|---------------|
| **Input Events** | | |
| `keyboard` | `KeyboardEvent` | channel_size=16 |
| `modifier` | `ModifierEvent` | |
| `pointing` | `PointingEvent` | channel_size=8 |
| **State Events** | | |
| `layer_change` | `LayerChangeEvent` | subs=4 |
| `wpm_update` | `WpmUpdateEvent` | |
| `led_indicator` | `LedIndicatorEvent` | |
| `sleep_state` | `SleepStateEvent` | |
| **Battery Events** | | |
| `battery_adc` | `BatteryAdcEvent` | channel_size=2 |
| `charging_state` | `ChargingStateEvent` | channel_size=2 |
| `battery_state` | `BatteryStateEvent` | subs=4 |
| **Connection Events** | | |
| `connection_change` | `ConnectionChangeEvent` | |
| `ble_state_change` | `BleStateChangeEvent` | |
| `ble_profile_change` | `BleProfileChangeEvent` | |
| **Split Events** | | |
| `peripheral_connected` | `PeripheralConnectedEvent` | |
| `central_connected` | `CentralConnectedEvent` | |
| `peripheral_battery` | `PeripheralBatteryEvent` | channel_size=2, subs=2 |
| `clear_peer` | `ClearPeerEvent` | |

## Related Documentation

- [Event](../features/event) - Event concepts, built-in events, and custom event definition
- [Input Device](../features/input_device) - How to create input devices that publish events
- [Processor](../features/processor) - How to create processors that subscribe to events
