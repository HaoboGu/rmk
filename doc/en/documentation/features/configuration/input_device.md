# Input devices

All input devices are defined in the `[input_device]` table. Currently supported input device types include:

- Rotary Encoder (encoder)
- Joystick (joystick)

## Rotary Encoder(not ready yet)

A rotary encoder is a common input device that can be used for volume control, page scrolling, and other functions. It can be defined in the configuration file as follows:

```toml
[[input_device.encoder]]
pin_a = "P0_30"
pin_b = "P0_31"

# Working mode of the encoder
# Available modes:
# - default: EC11 compatible mode, resolution = 1
# - e8h7: resolution = 2, direction reversed
# - resolution: custom resolution, requires specifying resolution and reverse parameters
phase = "default" 

# Resolution represents the number of pulses generated per detent
# For example: if your encoder has 30 detents and generates 15 pulses per 360-degree rotation, then resolution = 30/15 = 2
# The number of detents and pulses can be found in your encoder's datasheet
resolution = 2

# Whether to reverse the encoder direction
reverse = false
```

Multiple encoders can be added, and their indices are determined by the order of addition:

```toml
# Encoder 0
[[input_device.encoder]]
pin_a = "P0_01"
pin_b = "P0_02"
phase = "default" 

# Encoder 1
[[input_device.encoder]]
pin_a = "P0_03"
pin_b = "P0_04"
phase = "default" 
```

## Joystick

A joystick is an analog input device that can be used for mouse control and other functions. Currently only NRF series chips are supported.

```toml
[[input_device.joystick]]
name = "default"
pin_x = "P0_31"
pin_y = "P0_29"
pin_z = "_"
transform = [[80, 0], [0, 80]]
bias = [29130, 29365]
resolution = 6
```

### Parameters:

- `name`: Unique name for the joystick. If you have multiple joysticks, they need different names
- `pin_x`: Pin for X-axis
- `pin_y`: Pin for Y-axis
- `pin_z`: Pin for Z-axis
- `transform`: Transformation matrix for the joystick
- `bias`: Bias value for each axis
- `resolution`: Resolution for each axis

> #### Axis Configuration Note:
>
> `_` indicates that the axis does not exist.
>
> `_` is only allowed for:
> 1. Both y and z axes are missing
> 2. Only z axis is missing
>
> For example: `pin_x = "_"` `pin_y = "P0_29"` `pin_z = "P0_30"` is not allowed

### Working Principle

1. Device reads values from each axis
2. Adds `bias` value to each axis to make the value close to 0 when the joystick is released
3. About the `transform` matrix:
    1. New x-axis value = (axis_x + bias[0]) / transform[0][0] + (axis_y + bias[1]) / transform[0][1] + (axis_z + bias[2]) / transform[0][2]
    2. New y-axis value = (axis_x + bias[0]) / transform[1][0] + (axis_y + bias[1]) / transform[1][1] + (axis_z + bias[2]) / transform[1][2]
    3. New z-axis value = (axis_x + bias[0]) / transform[2][0] + (axis_y + bias[1]) / transform[2][1] + (axis_z + bias[2]) / transform[2][2]

    If `transform[new_axis][old_axis]` is 0, that old axis value is ignored.

    Since the value range read by the ADC device is usually much larger than the mouse report range of -256~255, `transform` is designed as a divisor.
4. Each axis value is adjusted to the largest integer multiple of `resolution` that is less than its original value to reduce noise from ADC device readings.

### Quick Configuration Guide

1. First set `bias` to 0, `resolution` to 1, and `transform` to `[[1, 0, 0], [0, 1, 0], [0, 0, 1]]` (matrix dimension depends on the number of axes)

2. Find the optimal `bias` value:
   - Use a debug probe to find the output `JoystickProcessor::generate_report: record = [axis_x, axis_y, axis_z]` in debug information
   - Observe these values to find the `bias` value that makes each axis closest to 0 when the joystick is released

3. If the mouse moves too fast, gradually increase the `transform` value until you find the right sensitivity

4. If the mouse jitters, gradually increase the `resolution` value until the jitter disappears

## Pointing Device(Draft, not implemented)

Pointing devices (such as touchpads) can be connected via I2C or SPI interface. Configuration examples:

```toml
[[input_device.pointing]]
interface = { i2c = { instance = "TWIM0", scl = "P0_27", sda = "P0_26" } }
```

or

```toml
[[input_device.pointing]]
interface = { spi = { instance = "SPIM0", sck = "P0_25", mosi = "P0_24", miso = "P0_23", cs = "P0_22", cpi = 1000 } }
```

### Parameters:

#### I2C Configuration
- `instance`: I2C instance name
- `scl`: Clock pin
- `sda`: Data pin

#### SPI Configuration
- `instance`: SPI instance name
- `sck`: Clock pin
- `mosi`: Master Out Slave In pin
- `miso`: Master In Slave Out pin
- `cs`: Chip Select pin (optional)
- `cpi`: Counts Per Inch (optional)
