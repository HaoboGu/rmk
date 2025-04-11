# Storage

## `[storage]`

`[storage]` section defines storage related configs. Storage feature is required to persist keymap data, it's strongly recommended to make it enabled(and it's enabled by default!). RMK will automatically use the last two section of chip's internal flash as the pre-served storage space. For some chips, there's also predefined default configuration, such as [nRF52840](https://github.com/HaoboGu/rmk/blob/main/rmk-macro/src/default_config/nrf52840.rs). If you don't want to change the default setting, just ignore this section.
```toml
[storage]
# Storage feature is enabled by default
enabled = true
# Start address of local storage, MUST BE start of a sector.
# If start_addr is set to 0(this is the default value), the last `num_sectors` sectors will be used.
start_addr = 0x00000000
# How many sectors are used for storage, the default value is 2
num_sectors = 2
```