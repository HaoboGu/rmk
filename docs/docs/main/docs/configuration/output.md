# Output

You can configure static output pins for your keyboard. For single keyboards, use `[[output]]`. For split keyboards, use `[[split.central.output]]` for the central board and `[[split.peripheral.output]]` for each peripheral.

```toml
# Output configuration, if you don't need to set an output pin, just ignore this section.
# Note the double brackets [[ ]], which indicate that multiple outputs can be defined.
[[output]]
# Only the pin name is required, the rest of the fields are optional
pin = "PIN_13"
initial_state_active = false
low_active = false

# Split central output configuration
[[split.central.output]]
# Only the pin name is required, the rest of the fields are optional
pin = "PIN_12"
initial_state_active = false
low_active = false

# Split peripheral output configuration
[[split.peripheral.output]]
# Only the pin name is required, the rest of the fields are optional
pin = "PIN_13"
initial_state_active = false
low_active = false


```
