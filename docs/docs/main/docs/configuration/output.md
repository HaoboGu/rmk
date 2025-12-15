# Output

You can configure static output pins for your keyboard. Depending on whether you are using a split or a single keyboard, you need to configure the output pins as an `[[output]]` element or as an `[[split.peripheral.output]]` element in your config file.

```toml
# Output configuration, if you don't neet to set an output pin, just ignore this section.
# Note the double brackets [[ ]], which indicate that multiple outputs can be defined.
[[output]]
# Only the pin name is required, the rest of the fields are optional
pin = "PIN_13"
initial_state_active = false
low_active = false

# Output configuration, if you don't neet to set an output pin, just ignore this section.
# Note the double brackets [[ ]], which indicate that multiple outputs can be defined.
[[split.peripheral.output]]
# Only the pin name is required, the rest of the fields are optional
pin = "PIN_13"
initial_state_active = false
low_active = false


```

