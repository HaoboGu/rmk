# Chip-Specific Configuration

The `[chip.<chip_name>]` section allows you to configure chip-specific settings for different microcontrollers. These settings are optional and only needed when you want to override the default behavior.

## nRF52840

### DCDC Configuration

The nRF52840 has two DC/DC regulators that can improve power efficiency when enabled with proper hardware support.

#### Configuration Example

```toml
[chip.nrf52840]
# Enable DCDC regulator 0
dcdc_reg0 = true
# Enable DCDC regulator 1
dcdc_reg1 = true
# Set DCDC regulator 0 voltage
dcdc_reg0_voltage = "3V3"  # Options: "3V3" or "1V8"
```

::: danger Hardware Requirement
Do not enable DC/DC regulators without an external LC filter being connected, as this will inhibit device operation, including debug access, until an LC filter is connected.
:::

## nRF52833

### DCDC Configuration

The nRF52833 has one DC/DC regulator available for configuration.

#### Configuration Example

```toml
[chip.nrf52833]
# Enable DCDC regulator 1
dcdc_reg1 = true
```

::: danger Hardware Requirement
Do not enable DC/DC regulator without an external LC filter being connected, as this will inhibit device operation, including debug access, until an LC filter is connected.
:::