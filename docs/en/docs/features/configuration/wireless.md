# Wireless/Bluetooth

### `[ble]`

To enable BLE, add `enabled = true` under the `[ble]` section.

There are several more configs for reading battery level and charging state, now they are available for nRF52840 only.

```toml
# Ble configuration
# To use the default configuration, ignore this section completely
[ble]
# Whether to enable BLE feature
enabled = true
# nRF52840's saadc pin for reading battery level, you can use a pin number or "vddh"
battery_adc_pin = "vddh"
# The voltage divider setting for saadc, this setting should be ignored when using "vddh" as the adc pin.
# For example, nice!nano have 806 + 2M resistors, the saadc measures voltage on 2M resistor, so the two values should be set to 2000 and 2806
adc_divider_measured = 2000
adc_divider_total = 2806
# Pin that reads battery's charging state, `low-active` means the battery is charging when `charge_state.pin` is low
charge_state = { pin = "PIN_1", low_active = true }
# Output LED pin that blinks when the battery is low
charge_led= { pin = "PIN_2", low_active = true }
```

::: warning

In current version, when using split, central and peripherals can only share the same ADC config. This issue will be fixed soon.

:::