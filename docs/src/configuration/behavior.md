# Behavior

## `[behavior]`

`[behavior]` section contains configuration for how different keyboard actions should behave:

```toml
[behavior]
tri_layer = { uppper = 1, lower = 2, adjust = 3 }
one_shot = { timeout = "1s" }
```

### Tri Layer

`Tri Layer` works by enabling a layer (called `adjust`) when other two layers (`upper` and `lower`) are both enabled.

You can enable Tri Layer by specifying the `upper`, `lower` and `adjust` layers in the `tri_layer` sub-table:

```toml
[behavior.tri_layer]
uppper = 1
lower = 2
adjust = 3
```
In this example, when both layers 1 (`upper`) and 2 (`lower`) are active, layer 3 (`adjust`) will also be enabled.

### Tap Hold

In the `tap_hold` sub-table, you can configure the following parameters:

- `enable_hrm`: Enables or disables HRM (Home Row Mod) mode. When enabled, the `prior_idle_time` setting becomes functional. Defaults to `false`.
- `prior_idle_time`: If the previous non-modifier key is released within this period before pressing the current tap-hold key, the tap action for the tap-hold behavior will be triggered. This parameter is effective only when enable_hrm is set to `true`. Defaults to 120ms.
- `hold_timeout`: Defines the duration a tap-hold key must be pressed to determine hold behavior. If tap-hold key is released within this time, the key is recognized as a "tap". Holding it beyond this duration triggers the "hold" action. Defaults to 250ms.
- `post_wait_time`: Adds an additional delay after releasing a tap-hold key to check if any keys pressed during the `hold_timeout` are released. This helps accommodate fast typing scenarios where some keys may not be fully released during a hold. Defaults to 50ms

The following are the typical configurations:

```toml
[behavior]
# Enable HRM 
tap_hold = { enable_hrm = true, prior_idle_time = "120ms", hold_timeout = "250ms", post_wait_time = "50ms"}
# Disable HRM, you can safely ignore any fields if you don't want to change them
tap_hold = { enable_hrm = false, hold_timeout = "200ms" }
```

### One Shot

In the `one_shot` sub-table you can define how long OSM or OSL will wait before releasing the modifier/layer with the `timeout` option, default is one second.
`timeout` is a string with a suffix of either "s" or "ms".

```toml
[behavior.one_shot]
timeout = "5s"
```

### Combo

In the `combo` sub-table, you can configure the keyboard's combo key functionality. Combo allows you to define a group of keys that, when pressed simultaneously, will trigger a specific output action.

Combo configuration includes the following parameters:

- `timeout`: Defines the maximum time window for pressing all combo keys. If the time exceeds this, the combo key will not be triggered. The format is a string, which can be milliseconds (e.g. "200ms") or seconds (e.g. "1s").
- `combos`: An array containing all defined combos. Each combo configuration is an object containing the following attributes:
  - `actions`: An array of strings defining the keys that need to be pressed simultaneously to trigger the combo action.
  - `output`: A string defining the output action to be triggered when all keys in `actions` are pressed simultaneously.
  - `layer`: An optional parameter, a number, specifying which layer the combo is valid on. If not specified, the combo is valid on all layers.

Here is an example of combo configuration:

```toml
[behavior.combo]
timeout = "150ms"
combos = [
  # Press J and K keys simultaneously to output Escape key
  { actions = ["J", "K"], output = "Escape" },
  # Press F and D keys simultaneously to output Tab key, but only valid on layer 0
  { actions = ["F", "D"], output = "Tab", layer = 0 },
  # Three-key combo, press A, S, and D keys to switch to layer 2
  { actions = ["A", "S", "D"], output = "TO(2)" }
]
```
