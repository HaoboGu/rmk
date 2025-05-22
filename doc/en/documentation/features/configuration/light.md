# Light

## `[light]`

`[light]` section defines lights of the keyboard, aka `capslock`, `scrolllock` and `numslock`. They are actually an input pin, so there are two fields available: `pin` and `low_active`.

`pin` field is just like IO pins in `[matrix]`, `low_active` defines whether the light low-active or high-active(`true` means low-active).

You can safely ignore any of them, or the whole `[light]` section if you don't need them.

```toml
[light]
capslock = { pin = "PIN_0", low_active = true }
scrolllock = { pin = "PIN_1", low_active = true }
numslock= { pin = "PIN_2", low_active = true }
```