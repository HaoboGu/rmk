# Input devices

All input devices are defined in the `[input_device]` table. Currently supported input device types include:

- [Rotary Encoder (encoder)](./encoder)
- [Joystick (joystick)](./joystick.md)
- [PMW3610 Optical Mouse Sensor (pmw3610)](./pmw3610.md)
- [IQS5xx Trackpad (iqs5xx)](./iqs5xx.md)

Please refer to the corresponding documentation for detailed configuration settings.

## Configuring Multiple Input Devices

You can define and configure any number of input devices. To add multiple instances of a device, simply repeat the device type sub-table in your configuration:

```toml
# Encoder 1
[[input_device.encoder]]

# Encoder 2
[[input_device.encoder]]

# Encoder ..

# JoyStick 1
[[input_device.joystick]]

# JoyStick 2
[[input_device.joystick]]

# JoyStick ..
```

## Input device in split keyboards

For split keyboard configurations, it is necessary to specify which part of the keyboard (the central or the peripheral) the input device is physically connected to.

For example, instead of using `[[input_device.encoder]]`, you should use:

- `[[split.central.input_device.encoder]]` to add an encoder to the central.
- `[[split.peripheral.input_device.encoder]]` to add an encoder to the peripheral.

::: note
If your keyboard has multiple peripherals, `[[split.peripheral.input_device.<device_type>]]` always refers to the input device on the nearest `[[split.peripheral]]`.
:::

The following is an example which shows how to organize input device configuration when there are multiple peripherals

```toml
[split]

# Split central
[split.central]

# Encoder 0 on the central
[[split.central.input_device.encoder]]

# Encoder 1 on the central
[[split.central.input_device.encoder]]

# Peripheral 0
[[split.peripheral]]

# Encoder 0 on periphreal 0
[[split.peripheral.input_device.encoder]]

# Encoder 1 on periphreal 0
[[split.peripheral.input_device.encoder]]

# Joystick 0 on periphreal 0
[[split.peripheral.input_device.joystick]]

# Peripheral 1
[[split.peripheral]]

# Encoder 0 on periphreal 1
[[split.peripheral.input_device.encoder]]

# Encoder 1 on periphreal 1
[[split.peripheral.input_device.encoder]]

```
