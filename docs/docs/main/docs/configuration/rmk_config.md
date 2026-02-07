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
# Maximum number of morse keys keyboard can store (max 256)
morse_max_num = 8
# Maximum number of patterns a morse key can handle (default: 8, min: 4, max 65536)
max_patterns_per_key = 8
# Macro space size in bytes for storing sequences. The maximum number of Macros depends on the size of each sequence: All sequences combined need to fit into macro_space_size, the number of macro sequences doesn't matter.
macro_space_size = 256
# Default debounce time in ms
debounce_time = 20
# Report channel size
report_channel_size = 16
# Vial channel size
vial_channel_size = 4
# Flash channel size
flash_channel_size = 4
# The number of the split peripherals
split_peripherals_num = 0
# The number of available BLE profiles
ble_profiles_num = 3
# BLE Split Central sleep timeout in seconds (0 = disabled)
split_central_sleep_timeout_seconds = 0
```

## Parameter Details

### Mouse-Related Configuration

- `mouse_key_interval`: Mouse key interval in milliseconds, default value is 20. This parameter controls the mouse movement speed; lower values result in faster movement.
- `mouse_wheel_interval`: Mouse wheel interval in milliseconds, default value is 80. This parameter controls the scrolling speed; lower values result in faster scrolling.

### Behavior Configuration

::: info

Increasing the number of combos, forks, morses (tap dances), and macros will increase memory usage.

:::

- `combo_max_num`: Maximum number of combos that the keyboard can store, default value is 8. This value must be between 0 and 256.
- `combo_max_length`: Maximum number of keys that can be pressed simultaneously in a combo, default value is 4.
- `fork_max_num`: Maximum number of forks for conditional key actions, default value is 8. This value must be between 0 and 256.
- `morse_max_num`: Maximum number of morses that can be stored, default value is 8. This value must be between 0 and 256.
- `max_patterns_per_key` : Maximum number of tap/hold patterns a morse key can handle, default value is 8. This value must be between 4 and 65536. (Will be automatically set to the maximum length of `tap_actions` + `hold_actions` or `morse_actions`.)
- `macro_space_size`: Space size in bytes for storing macro sequences, default value is 256.

### Matrix Configuration

- `debounce_time`: Default key debounce time in milliseconds, default value is 20.

### Channel Configuration

In RMK there are several channels used for communication between tasks. The length of the channel can be adjusted. Larger channel size means more events can be buffered, but it will increase memory usage.

- `report_channel_size`: The length of report channel, default value is 16. Used for buffering HID reports to be sent to the host.
- `vial_channel_size`: The length of vial channel, default value is 4. Used for communication with Vial protocol.
- `flash_channel_size`: The length of flash channel, default value is 4. Used for buffering flash storage operations.

### Split Keyboard Configuration

- `split_peripherals_num`: The number of split peripherals, default value is 0. If peripherals are specified in `keyboard.toml`, this value is automatically set to the actual count. If you're using the Rust API without `[[split.peripheral]]` entries, set this manually to match your peripheral count.

### Wireless Configuration

- `ble_profiles_num`: The number of available Bluetooth profiles, default value is 3. This parameter defines how many Bluetooth paired devices the keyboard can store.
- `split_central_sleep_timeout_seconds`: Sleep timeout for BLE split central in seconds, default value is 0 (disabled). When set to a non-zero value, the split central will enter sleep mode after this many seconds of inactivity to save power. Set to 0 to disable automatic sleep.
