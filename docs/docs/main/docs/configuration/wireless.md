# Wireless/Bluetooth

### `[ble]`

To enable BLE, add `enabled = true` under the `[ble]` section.

There are several more configs for reading battery level and charging state; they are currently available for nRF52 (SAADC) chips.

```toml
# Ble configuration
# To use the default configuration, ignore this section completely
[ble]
# Whether to enable BLE feature
enabled = true
# nRF52 SAADC pin for reading battery level, you can use a pin number or "vddh"
battery_adc_pin = "vddh"
# The voltage divider setting for saadc. This setting should be ignored when using "vddh" as the adc pin.
# For example, nice!nano has 806 + 2M resistors. The saadc measures voltage on the 2M resistor, so the two values should be set to 2000 and 2806
adc_divider_measured = 2000
adc_divider_total = 2806
# Set the BLE tx power; higher means better signal but more power consumption. For nRF52840 the maximum tx power is 8.
default_tx_power = 0
# Whether to enable 2M PHY, defaults to true.
use_2m_phy = true
# [Deprecated] Pin that reads battery's charging state, `low-active` means the battery is charging when `charge_state.pin` is low
# charge_state = { pin = "PIN_1", low_active = true }
# [Deprecated] Output LED pin that blinks when the battery is low
# charge_led= { pin = "PIN_2", low_active = true }
```

### Split battery ADC configuration

For split keyboards, you can configure battery ADC separately for the central and each peripheral:

```toml
[split.central]
battery_adc_pin = "P0_01"
adc_divider_measured = 2000
adc_divider_total = 2806

[[split.peripheral]]
battery_adc_pin = "P0_02"
adc_divider_measured = 2000
adc_divider_total = 2806
```

Notes:
- If `[split.central]` provides battery ADC settings, they override the top-level `[ble]` battery settings for the central.
- Peripherals do **not** fall back to `[ble]`; to enable peripheral battery reporting, set ADC values per peripheral.
