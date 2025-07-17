# RMK Internal Configuration

The `[rmk]` section defines configuration parameters used inside RMK. These parameters affect the firmware's behavior, memory usage, and performance. If you don't need to change these parameters, you can ignore this section.

## Configuration Example

```toml
[rmk]
# Mouse key interval (ms) - controls mouse movement speed
mouse_key_interval = 20
# Mouse wheel interval (ms) - controls scrolling speed
mouse_wheel_interval = 80
# Maximum number of combos keyboard can store
combo_max_num = 8
# Maximum number of keys pressed simultaneously in a combo
combo_max_length = 4
# Maximum number of forks for conditional key actions
fork_max_num = 8
# Maximum number of tap dances keyboard can store
tap_dance_max_num = 8
# Maximum number of taps per tap dance (default: 2, min: 2, max: 256)
tap_dance_max_tap = 2
# Macro space size in bytes for storing sequences. The maximum number of Macros depends on the size of each sequence: All sequences combined need to fit into macro_space_size, the number of macro sequences doesn't matter.
macro_space_size = 256
# Default debounce time in ms
debounce_time = 20
# Event channel size
event_channel_size = 16
# Report channel size
report_channel_size = 16
# Vial channel size
vial_channel_size = 4
# Flash channel size
flash_channel_size = 4
# The number of the split peripherals
split_peripherals_num = 1
# The size of the split message channel
split_message_channel_size = 4
# The number of available BLE profiles
ble_profiles_num = 3
```

## Parameter Details

### Mouse-Related Configuration

- `mouse_key_interval`: Mouse key interval in milliseconds, default value is 20. This parameter controls the mouse movement speed; lower values result in faster movement.
- `mouse_wheel_interval`: Mouse wheel interval in milliseconds, default value is 80. This parameter controls the scrolling speed; lower values result in faster scrolling.

### Behavior Configuration

::: info

Increasing the number of combos, forks, tap dances and macros will increase memory usage.

:::

- `combo_max_num`: Maximum number of combos that the keyboard can store, default value is 8. This value must be between 0 and 256.
- `combo_max_length`: Maximum number of keys that can be pressed simultaneously in a combo, default value is 4.
- `fork_max_num`: Maximum number of forks for conditional key actions, default value is 8. This value must be between 0 and 256.
- `tap_dance_max_num`: Maximum number of tap dances that can be stored, default value is 8. This value must be between 0 and 256.
- `tap_dance_max_tap`: Maximum number of taps per tap dance, default value is 2. This value must be between 2 and 256. If `tap_actions` or `hold_actions` in [tap-dance config](./behavior.md#tap-dance) is set, the `tap_dance_max_tap` will be automatically set to the maximum length of `tap_actions` or `hold_actions`.
- `macro_space_size`: Space size in bytes for storing macro sequences, default value is 256.

### Matrix Configuration

- `debounce_time`: Default key debounce time in milliseconds, default value is 20.

### Channel Configuration

In RMK there are several channels used for communication between tasks. The length of the channel can be adjusted. Larger channel size means more events can be buffered, but it will increase memory usage.

- `event_channel_size`: The length of event channel, default value is 16.
- `report_channel_size`: The length of report channel, default value is 16.
- `vial_channel_size`: The length of vial channel, default value is 4.
- `flash_channel_size`: The length of flash channel, default value is 4.

### Split Keyboard Configuration

- `split_peripherals_num`: The number of split peripherals, default value is 1. If multiple peripherals are specified in the toml, this field will be automatically set to the actual peripherals number.
- `split_message_channel_size`: The length of the split message channel, default value is 4.

### Wireless Configuration

- `ble_profiles_num`: The number of available Bluetooth profiles, default value is 3. This parameter defines how many Bluetooth paired devices the keyboard can store.
